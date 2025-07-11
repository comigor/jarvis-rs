use crate::{
    config::{LlmConfig, McpServerConfig},
    history::{HistoryStorage, Message},
    llm::{ChatMessage, LlmClient, OpenAiClient, Tool, Function},
    mcp::{
        create_mcp_client, McpClient, McpClientCapabilities, McpInitializeRequest,
        McpRootsCapability,
    },
    Error, Result,
};
use super::fsm::{AgentEvent, AgentState, AgentStateMachine};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct Agent {
    llm_client: Box<dyn LlmClient>,
    mcp_clients: HashMap<String, Box<dyn McpClient>>,
    available_tools: Vec<Tool>,
    discovered_prompts: Vec<String>,
    default_system_prompt: String,
    base_system_prompt: Option<String>,
}

impl Agent {
    pub async fn new(
        llm_config: LlmConfig,
        mcp_configs: Vec<McpServerConfig>,
    ) -> Result<Self> {
        info!("Initializing agent with {} MCP servers", mcp_configs.len());

        // Initialize LLM client
        let llm_client = Box::new(OpenAiClient::new(llm_config.clone()));

        // Initialize MCP clients
        let mut mcp_clients = HashMap::new();
        let mut available_tools = Vec::new();
        let mut discovered_prompts = Vec::new();

        for config in mcp_configs {
            match Self::initialize_mcp_client(config).await {
                Ok((name, mut client, tools, prompts)) => {
                    // Store tools
                    for tool in tools {
                        let llm_tool = Tool {
                            tool_type: "function".to_string(),
                            function: Function {
                                name: tool.name.clone(),
                                description: tool.description,
                                parameters: tool.input_schema,
                            },
                        };
                        available_tools.push(llm_tool);
                    }

                    // Store prompts
                    discovered_prompts.extend(prompts);

                    // Store client
                    mcp_clients.insert(name, client);
                }
                Err(e) => {
                    warn!("Failed to initialize MCP client: {}", e);
                    continue;
                }
            }
        }

        let default_system_prompt = 
            "You are a helpful AI assistant. Please respond to the user's request accurately and concisely.".to_string();

        info!(
            "Agent initialized with {} MCP clients, {} tools, {} discovered prompts",
            mcp_clients.len(),
            available_tools.len(),
            discovered_prompts.len()
        );

        Ok(Self {
            llm_client,
            mcp_clients,
            available_tools,
            discovered_prompts,
            default_system_prompt,
            base_system_prompt: llm_config.system_prompt,
        })
    }

    async fn initialize_mcp_client(
        config: McpServerConfig,
    ) -> Result<(String, Box<dyn McpClient>, Vec<crate::mcp::McpTool>, Vec<String>)> {
        debug!("Initializing MCP client: {}", config.name);

        let mut client = create_mcp_client(config.clone()).await?;

        // Initialize the client
        let init_request = McpInitializeRequest {
            capabilities: McpClientCapabilities {
                roots: Some(McpRootsCapability {
                    list_changed: false,
                }),
                sampling: None,
            },
        };

        let init_response = client.initialize(init_request).await?;
        info!("MCP client '{}' initialized successfully", config.name);

        // Discover tools
        let tools = match client.list_tools().await {
            Ok(tools) => {
                debug!("Discovered {} tools from MCP client '{}'", tools.len(), config.name);
                tools
            }
            Err(e) => {
                warn!("Failed to list tools from MCP client '{}': {}", config.name, e);
                Vec::new()
            }
        };

        // Discover prompts (system prompts)
        let mut prompts = Vec::new();
        if init_response.capabilities.prompts.is_some() {
            match client.list_prompts().await {
                Ok(prompt_list) => {
                    // Find prompts with no arguments (system prompts)
                    for prompt in prompt_list {
                        if prompt.arguments.is_empty() {
                            match client
                                .get_prompt(crate::mcp::McpGetPromptRequest {
                                    name: prompt.name.clone(),
                                    arguments: HashMap::new(),
                                })
                                .await
                            {
                                Ok(prompt_response) => {
                                    // Look for assistant messages in the prompt
                                    for message in prompt_response.messages {
                                        if message.role == "assistant" {
                                            if let crate::mcp::McpContent::Text { text } = message.content {
                                                prompts.push(text);
                                                info!("Discovered system prompt from MCP client '{}': {}", config.name, prompt.name);
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to get prompt '{}' from MCP client '{}': {}", prompt.name, config.name, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to list prompts from MCP client '{}': {}", config.name, e);
                }
            }
        }

        Ok((config.name, client, tools, prompts))
    }

    pub async fn process(
        &mut self,
        session_id: &str,
        input: &str,
        history: &HistoryStorage,
    ) -> Result<String> {
        info!("Processing request for session: {}", session_id);

        // Generate final system prompt
        let final_system_prompt = self.build_system_prompt();

        // Retrieve message history
        let previous_messages = history.list(session_id).await?;
        debug!("Retrieved {} previous messages for session", previous_messages.len());

        // Build initial messages
        let mut messages = Vec::new();

        // Add system prompt if available
        if !final_system_prompt.is_empty() {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: final_system_prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        // Add previous messages
        for msg in previous_messages {
            messages.push(ChatMessage {
                role: msg.role,
                content: msg.content,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        // Add current user input
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: input.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Save user message to history
        let user_message = Message::user(session_id.to_string(), input.to_string());
        history.save(user_message).await?;

        // Create FSM with initial state
        let mut fsm = AgentStateMachine::new(
            messages,
            self.available_tools.clone(),
            // Note: We can't move mcp_clients here due to borrowing rules
            // In a real implementation, you'd use Arc<Mutex<>> or similar
            HashMap::new(), // Placeholder for now
        );

        // Process through FSM until terminal state
        let mut result = self.run_fsm_loop(&mut fsm).await?;

        // Save assistant response to history
        let assistant_message = Message::assistant(session_id.to_string(), result.clone());
        history.save(assistant_message).await?;

        Ok(result)
    }

    async fn run_fsm_loop(&mut self, fsm: &mut AgentStateMachine) -> Result<String> {
        // Initial event to start processing
        fsm.process_event(AgentEvent::ProcessInput, self.llm_client.as_ref()).await?;

        // Main FSM loop
        while !fsm.is_terminal() {
            match fsm.current_state() {
                AgentState::AwaitingLlmResponse => {
                    // Check if LLM requested tools or provided content
                    if let Some(ref response) = &fsm.context.llm_response {
                        if !response.choices.is_empty() {
                            let choice = &response.choices[0];
                            if choice.message.tool_calls.is_some() && !choice.message.tool_calls.as_ref().unwrap().is_empty() {
                                fsm.process_event(AgentEvent::LlmRequestedTools, self.llm_client.as_ref()).await?;
                            } else {
                                fsm.process_event(AgentEvent::LlmRespondedWithContent, self.llm_client.as_ref()).await?;
                            }
                        }
                    }
                }
                AgentState::ExecutingTools => {
                    // Prepare tool execution
                    let tool_calls = fsm.prepare_tool_execution();
                    
                    // Execute tools
                    let mut results = Vec::new();
                    for tool_call in &tool_calls {
                        let result = self.execute_tool(tool_call).await;
                        results.push((tool_call.id.clone(), result));
                    }
                    
                    // Add results back to FSM
                    fsm.add_tool_execution_results(results);
                    
                    // Continue with tools execution completed
                    fsm.process_event(AgentEvent::ToolsExecutionCompleted, self.llm_client.as_ref()).await?;
                }
                AgentState::ReadyToCallLlm => {
                    // Make another LLM call
                    fsm.process_event(AgentEvent::ProcessInput, self.llm_client.as_ref()).await?;
                }
                _ => {
                    break;
                }
            }
        }

        // Return result based on final state
        match fsm.current_state() {
            AgentState::Done => Ok(fsm.get_final_content().to_string()),
            AgentState::Error => {
                if let Some(error) = fsm.get_last_error() {
                    Err(error.clone())
                } else {
                    Err(Error::internal("FSM ended in error state without specific error"))
                }
            }
            _ => Err(Error::internal(format!(
                "FSM ended in unexpected state: {:?}",
                fsm.current_state()
            ))),
        }
    }

    fn build_system_prompt(&self) -> String {
        let mut prompt_parts = Vec::new();

        // Start with base system prompt
        let base = self.base_system_prompt.as_ref()
            .unwrap_or(&self.default_system_prompt);
        prompt_parts.push(base.clone());

        // Add discovered MCP prompts
        for mcp_prompt in &self.discovered_prompts {
            prompt_parts.push(mcp_prompt.clone());
        }

        prompt_parts.join("\n\n")
    }

    async fn execute_tool(&mut self, tool_call: &crate::llm::ToolCall) -> String {
        debug!("Executing tool: {}", tool_call.function.name);

        // Parse tool arguments
        let arguments: HashMap<String, serde_json::Value> = match serde_json::from_str(&tool_call.function.arguments) {
            Ok(args) => args,
            Err(e) => {
                error!("Failed to parse tool arguments: {}", e);
                return format!("Error: Could not parse arguments for tool {}: {}", tool_call.function.name, e);
            }
        };

        // Find the appropriate MCP client
        // For now, just use the first available client
        if let Some((_, client)) = self.mcp_clients.iter_mut().next() {
            let request = crate::mcp::McpToolCallRequest {
                name: tool_call.function.name.clone(),
                arguments,
            };

            match client.call_tool(request).await {
                Ok(response) => {
                    if response.is_error {
                        warn!("MCP tool execution failed: {:?}", response.content);
                    }

                    // Extract text content from response
                    for content in response.content {
                        if let crate::mcp::McpContent::Text { text } = content {
                            return text;
                        }
                    }

                    "Tool executed successfully, but no text content returned".to_string()
                }
                Err(e) => {
                    error!("MCP tool call failed: {}", e);
                    format!("Error executing tool {}: {}", tool_call.function.name, e)
                }
            }
        } else {
            format!("Error: No MCP client available for tool {}", tool_call.function.name)
        }
    }
}