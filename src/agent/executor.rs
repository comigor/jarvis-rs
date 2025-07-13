use super::fsm::{AgentEvent, AgentState, AgentStateMachine};
use crate::{
    Error, Result,
    config::{LlmConfig, McpServerConfig},
    history::{HistoryStorage, Message},
    llm::{ChatMessage, Function, LlmClient, OpenAiClient, Tool},
    mcp::{
        McpClient, McpClientCapabilities, McpInitializeRequest, McpRootsCapability,
        create_mcp_client,
    },
};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

pub struct Agent {
    llm_client: Box<dyn LlmClient>,
    mcp_clients: HashMap<String, Box<dyn McpClient>>,
    available_tools: Vec<Tool>,
    tool_to_client_map: HashMap<String, String>, // Maps tool_name -> client_name
    discovered_prompts: Vec<String>,
    default_system_prompt: String,
    base_system_prompt: Option<String>,
}

impl Agent {
    pub async fn new(llm_config: LlmConfig, mcp_configs: Vec<McpServerConfig>) -> Result<Self> {
        info!("Initializing agent with {} MCP servers", mcp_configs.len());

        // Initialize LLM client
        let llm_client = Box::new(OpenAiClient::new(llm_config.clone()));

        // Initialize MCP clients
        let mut mcp_clients = HashMap::new();
        let mut available_tools = Vec::new();
        let mut tool_to_client_map = HashMap::new();
        let mut discovered_prompts = Vec::new();

        for config in mcp_configs {
            match Self::initialize_mcp_client(config).await {
                Ok((name, client, tools, prompts)) => {
                    // Store tools and create tool-to-client mapping
                    for tool in tools {
                        let tool_name = tool.name.clone();

                        // Check for tool name conflicts
                        if let Some(existing_client) = tool_to_client_map.get(&tool_name) {
                            warn!(
                                "Tool name conflict: '{}' exists in both '{}' and '{}' clients. Using '{}'",
                                tool_name, existing_client, name, name
                            );
                        }

                        // Map tool name to client name
                        tool_to_client_map.insert(tool_name.clone(), name.clone());

                        let llm_tool = Tool {
                            tool_type: "function".to_string(),
                            function: Function {
                                name: tool_name,
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
            "Agent initialized with {} MCP clients, {} tools ({} mappings), {} discovered prompts",
            mcp_clients.len(),
            available_tools.len(),
            tool_to_client_map.len(),
            discovered_prompts.len()
        );

        Ok(Self {
            llm_client,
            mcp_clients,
            available_tools,
            tool_to_client_map,
            discovered_prompts,
            default_system_prompt,
            base_system_prompt: llm_config.system_prompt,
        })
    }

    async fn initialize_mcp_client(
        config: McpServerConfig,
    ) -> Result<(
        String,
        Box<dyn McpClient>,
        Vec<crate::mcp::McpTool>,
        Vec<String>,
    )> {
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
                debug!(
                    "Discovered {} tools from MCP client '{}'",
                    tools.len(),
                    config.name
                );
                tools
            }
            Err(e) => {
                warn!(
                    "Failed to list tools from MCP client '{}': {}",
                    config.name, e
                );
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
                                            if let crate::mcp::McpContent::Text { text } =
                                                message.content
                                            {
                                                prompts.push(text);
                                                info!(
                                                    "Discovered system prompt from MCP client '{}'",
                                                    config.name
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to get prompt '{}' from MCP client '{}': {}",
                                        prompt.name, config.name, e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to list prompts from MCP client '{}': {}",
                        config.name, e
                    );
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
        debug!(
            "Retrieved {} previous messages for session",
            previous_messages.len()
        );

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
        let result = self.run_fsm_loop(&mut fsm).await?;

        // Save assistant response to history
        let assistant_message = Message::assistant(session_id.to_string(), result.clone());
        history.save(assistant_message).await?;

        Ok(result)
    }

    async fn run_fsm_loop(&mut self, fsm: &mut AgentStateMachine) -> Result<String> {
        let start_time = std::time::Instant::now();
        info!("🚀 Starting FSM loop");
        let mut loop_iteration = 0;

        // Initial event to start processing
        debug!("🎬 Sending initial ProcessInput event");
        let event_start = std::time::Instant::now();
        fsm.process_event(AgentEvent::ProcessInput, Some(self.llm_client.as_ref()))
            .await?;
        debug!(
            "⏱️ Initial ProcessInput event took {:?}",
            event_start.elapsed()
        );

        // Main FSM loop
        info!("🔄 Entering main FSM loop");
        while !fsm.is_terminal() {
            loop_iteration += 1;
            debug!(
                "🔄 FSM loop iteration {} - current state: {:?}",
                loop_iteration,
                fsm.current_state()
            );

            if loop_iteration > 5 {
                error!(
                    "🚨 FSM loop iteration limit exceeded ({}), breaking to prevent infinite loop",
                    loop_iteration
                );
                break;
            }
            match fsm.current_state() {
                AgentState::AwaitingLlmResponse => {
                    // Check if we need to make an LLM call or process existing response
                    if fsm.context.llm_response.is_none() {
                        // Make LLM call
                        debug!(
                            "🤖 Making LLM call with {} messages",
                            fsm.context.messages.len()
                        );

                        let chat_request = crate::llm::ChatCompletionRequest {
                            model: "".to_string(), // Model will be set by the LLM client
                            messages: fsm.context.messages.clone(),
                            tools: self.available_tools.clone(),
                            temperature: None,
                            max_tokens: None,
                        };

                        let llm_start = std::time::Instant::now();
                        match self.llm_client.create_chat_completion(chat_request).await {
                            Ok(response) => {
                                let llm_duration = llm_start.elapsed();
                                info!(
                                    "✅ LLM responded with {} choices in {:?}",
                                    response.choices.len(),
                                    llm_duration
                                );
                                fsm.context.llm_response = Some(response);
                            }
                            Err(e) => {
                                error!("❌ LLM call failed: {}", e);
                                fsm.process_event(
                                    AgentEvent::ErrorOccurred,
                                    Some(self.llm_client.as_ref()),
                                )
                                .await?;
                                continue;
                            }
                        }
                    }

                    // Process the LLM response
                    if let Some(response) = &fsm.context.llm_response {
                        if !response.choices.is_empty() {
                            let choice = &response.choices[0];
                            if choice.message.tool_calls.is_some()
                                && !choice.message.tool_calls.as_ref().unwrap().is_empty()
                            {
                                let tool_calls = choice.message.tool_calls.as_ref().unwrap();
                                debug!("🔧 LLM requested {} tool calls", tool_calls.len());

                                // Add the LLM's assistant message with tool_calls to conversation
                                debug!(
                                    "📝 Adding LLM assistant message with tool_calls to conversation"
                                );
                                fsm.context.messages.push(ChatMessage {
                                    role: "assistant".to_string(),
                                    content: choice.message.content.clone(),
                                    tool_calls: choice.message.tool_calls.clone(),
                                    tool_call_id: None,
                                    name: None,
                                });

                                // Convert LLM tool calls to MCP tool call requests and store ID mapping
                                let mut mcp_tool_calls = Vec::new();
                                let mut tool_call_ids = Vec::new();
                                for tool_call in tool_calls {
                                    let arguments: std::collections::HashMap<
                                        String,
                                        serde_json::Value,
                                    > = serde_json::from_str(&tool_call.function.arguments)
                                        .unwrap_or_default();

                                    mcp_tool_calls.push(crate::mcp::McpToolCallRequest {
                                        name: tool_call.function.name.clone(),
                                        arguments,
                                    });

                                    // Store the original LLM tool call ID
                                    tool_call_ids.push(tool_call.id.clone());
                                }

                                // Store tool calls and ID mapping for execution
                                fsm.context.pending_tool_calls = mcp_tool_calls;
                                fsm.context.tool_call_id_mapping = tool_call_ids;
                                debug!(
                                    "📋 Prepared {} MCP tool calls for execution",
                                    fsm.context.pending_tool_calls.len()
                                );

                                fsm.process_event(
                                    AgentEvent::LlmRequestedTools,
                                    Some(self.llm_client.as_ref()),
                                )
                                .await?;
                            } else {
                                debug!("💬 LLM provided content response");

                                // Add the LLM's response as an assistant message to conversation
                                if !choice.message.content.is_empty() {
                                    debug!(
                                        "📝 Adding LLM response content to conversation: {}",
                                        choice.message.content
                                    );
                                    fsm.context.messages.push(ChatMessage {
                                        role: "assistant".to_string(),
                                        content: choice.message.content.clone(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                        name: None,
                                    });
                                }

                                fsm.process_event(
                                    AgentEvent::LlmRespondedWithContent,
                                    Some(self.llm_client.as_ref()),
                                )
                                .await?;
                            }
                        } else {
                            warn!("⚠️ LLM response has no choices");
                            fsm.process_event(
                                AgentEvent::ErrorOccurred,
                                Some(self.llm_client.as_ref()),
                            )
                            .await?;
                        }
                    }
                }
                AgentState::ExecutingTools => {
                    debug!("🔧 Executing tools state");

                    // Prepare tool execution
                    let tool_calls = fsm.prepare_tool_execution();
                    info!("🛠️ Executing {} tool calls", tool_calls.len());

                    // Execute tools
                    let mut results = Vec::new();
                    let tools_start = std::time::Instant::now();
                    for (i, tool_call) in tool_calls.iter().enumerate() {
                        debug!(
                            "🔨 Executing tool {}/{}: {}",
                            i + 1,
                            tool_calls.len(),
                            tool_call.name
                        );
                        let tool_start = std::time::Instant::now();
                        let result = self.execute_mcp_tool(tool_call).await;
                        let tool_duration = tool_start.elapsed();
                        debug!(
                            "✅ Tool {} completed with {} content items in {:?}",
                            tool_call.name,
                            result.content.len(),
                            tool_duration
                        );
                        results.push(result);
                    }
                    let total_tools_duration = tools_start.elapsed();

                    info!(
                        "🎯 All {} tools completed in {:?}, adding results to FSM",
                        results.len(),
                        total_tools_duration
                    );
                    // Add results back to FSM
                    fsm.add_tool_execution_results(results);

                    // Continue with tools execution completed
                    debug!("📤 Sending ToolsExecutionCompleted event");
                    fsm.process_event(
                        AgentEvent::ToolsExecutionCompleted,
                        Some(self.llm_client.as_ref()),
                    )
                    .await?;
                }
                AgentState::ReadyToCallLlm => {
                    // Clear previous LLM response and prepare for new call
                    debug!("🔄 Ready to call LLM - clearing previous response");
                    fsm.context.llm_response = None;

                    // If we have tool results, add them to messages
                    if !fsm.context.tool_call_results.is_empty() {
                        debug!(
                            "📝 Adding {} tool results to conversation",
                            fsm.context.tool_call_results.len()
                        );
                        for (index, tool_result) in fsm.context.tool_call_results.iter().enumerate()
                        {
                            if let Some(crate::mcp::McpContent::Text { text }) =
                                tool_result.content.first()
                            {
                                // Get the corresponding tool call ID from the mapping
                                let tool_call_id = fsm
                                    .context
                                    .tool_call_id_mapping
                                    .get(index)
                                    .cloned()
                                    .unwrap_or_else(|| {
                                        warn!("Missing tool call ID mapping for index {}", index);
                                        format!("tool_call_{index}")
                                    });

                                debug!("📝 Adding tool result for tool_call_id: {}", tool_call_id);
                                fsm.context.messages.push(ChatMessage {
                                    role: "tool".to_string(),
                                    content: text.clone(),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call_id),
                                    name: None,
                                });
                            }
                        }
                        fsm.context.tool_call_results.clear();
                        fsm.context.tool_call_id_mapping.clear();
                    }

                    // Make another LLM call
                    fsm.process_event(AgentEvent::ProcessInput, Some(self.llm_client.as_ref()))
                        .await?;
                }
                _ => {
                    break;
                }
            }
        }

        // Return result based on final state
        let total_duration = start_time.elapsed();
        info!(
            "🏁 FSM loop completed in {} iterations, total duration: {:?}",
            loop_iteration, total_duration
        );

        match fsm.current_state() {
            AgentState::Done => {
                info!("✅ FSM completed successfully in state: Done");
                Ok(fsm.get_final_content().to_string())
            }
            AgentState::Error => {
                error!("❌ FSM ended in error state after {:?}", total_duration);
                if let Some(error) = fsm.get_last_error() {
                    Err(Error::internal(error))
                } else {
                    Err(Error::internal(
                        "FSM ended in error state without specific error",
                    ))
                }
            }
            _ => {
                warn!(
                    "⚠️ FSM ended in unexpected state: {:?} after {:?}",
                    fsm.current_state(),
                    total_duration
                );
                Err(Error::internal(format!(
                    "FSM ended in unexpected state: {:?}",
                    fsm.current_state()
                )))
            }
        }
    }

    fn build_system_prompt(&self) -> String {
        let mut prompt_parts = Vec::new();

        // Start with base system prompt
        let base = self
            .base_system_prompt
            .as_ref()
            .unwrap_or(&self.default_system_prompt);
        prompt_parts.push(base.clone());

        // Add discovered MCP prompts
        for mcp_prompt in &self.discovered_prompts {
            prompt_parts.push(mcp_prompt.clone());
        }

        prompt_parts.join("\n\n")
    }

    pub async fn execute_mcp_tool_for_testing(
        &mut self,
        tool_call: &crate::mcp::McpToolCallRequest,
    ) -> crate::mcp::McpToolCallResponse {
        self.execute_mcp_tool(tool_call).await
    }

    async fn execute_mcp_tool(
        &mut self,
        tool_call: &crate::mcp::McpToolCallRequest,
    ) -> crate::mcp::McpToolCallResponse {
        debug!("Executing MCP tool: {}", tool_call.name);

        // Find the appropriate MCP client using the tool-to-client mapping
        match self.tool_to_client_map.get(&tool_call.name) {
            Some(client_name) => {
                debug!(
                    "Tool '{}' mapped to client '{}'",
                    tool_call.name, client_name
                );

                match self.mcp_clients.get_mut(client_name) {
                    Some(client) => {
                        debug!(
                            "Executing tool '{}' on client '{}'",
                            tool_call.name, client_name
                        );
                        match client.call_tool(tool_call.clone()).await {
                            Ok(response) => {
                                debug!(
                                    "Tool '{}' executed successfully on client '{}' with {} content items",
                                    tool_call.name,
                                    client_name,
                                    response.content.len()
                                );
                                response
                            }
                            Err(e) => {
                                error!(
                                    "Tool '{}' execution failed on client '{}': {}",
                                    tool_call.name, client_name, e
                                );
                                crate::mcp::McpToolCallResponse {
                                    content: vec![crate::mcp::McpContent::Text {
                                        text: format!("Error: Tool execution failed: {e}"),
                                    }],
                                    is_error: true,
                                }
                            }
                        }
                    }
                    None => {
                        error!(
                            "Client '{}' for tool '{}' is no longer available",
                            client_name, tool_call.name
                        );
                        crate::mcp::McpToolCallResponse {
                            content: vec![crate::mcp::McpContent::Text {
                                text: format!(
                                    "Error: Client '{}' for tool '{}' is no longer available",
                                    client_name, tool_call.name
                                ),
                            }],
                            is_error: true,
                        }
                    }
                }
            }
            None => {
                error!("No client mapping found for tool: '{}'", tool_call.name);
                warn!(
                    "Available tools: {:?}",
                    self.tool_to_client_map.keys().collect::<Vec<_>>()
                );
                crate::mcp::McpToolCallResponse {
                    content: vec![crate::mcp::McpContent::Text {
                        text: format!(
                            "Error: No client mapping found for tool: '{}'. Available tools: {}",
                            tool_call.name,
                            self.tool_to_client_map
                                .keys()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    }],
                    is_error: true,
                }
            }
        }
    }

    // Test-specific methods for enabling proper testing
    pub fn new_for_testing(
        llm_client: Box<dyn LlmClient>,
        mcp_clients: HashMap<String, Box<dyn McpClient>>,
        tool_to_client_map: HashMap<String, String>,
        available_tools: Vec<Tool>,
    ) -> Self {
        Self {
            llm_client,
            mcp_clients,
            available_tools,
            tool_to_client_map,
            discovered_prompts: Vec::new(),
            default_system_prompt: "You are a helpful assistant.".to_string(),
            base_system_prompt: None,
        }
    }

    pub fn get_tool_to_client_map(&self) -> &HashMap<String, String> {
        &self.tool_to_client_map
    }

    pub fn get_available_tools(&self) -> &Vec<Tool> {
        &self.available_tools
    }

    pub fn get_mcp_clients(&self) -> &HashMap<String, Box<dyn McpClient>> {
        &self.mcp_clients
    }

    pub fn remove_mcp_client(&mut self, client_name: &str) -> Option<Box<dyn McpClient>> {
        self.mcp_clients.remove(client_name)
    }
}
