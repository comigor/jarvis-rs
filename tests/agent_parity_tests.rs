use async_trait::async_trait;
use jarvis_rust::{
    Error, Result,
    agent::Agent,
    config::LlmConfig,
    history::HistoryStorage,
    llm::{
        ChatCompletionChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
        Function as LlmFunction, FunctionCall, LlmClient, Tool, ToolCall,
    },
    mcp::{McpContent, McpToolCallRequest, McpToolCallResponse},
};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// Mock LLM client for testing
struct MockLlmClient {
    responses: Arc<Mutex<Vec<ChatCompletionResponse>>>,
    error: Option<String>,
    received_tools: Arc<Mutex<Vec<Tool>>>,
}

impl MockLlmClient {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            error: None,
            received_tools: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_response(&self, response: ChatCompletionResponse) {
        self.responses.lock().unwrap().push(response);
    }

    fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    fn get_received_tools(&self) -> Vec<Tool> {
        self.received_tools.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn create_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        // Store received tools for verification
        self.received_tools.lock().unwrap().clear();
        self.received_tools.lock().unwrap().extend(request.tools);

        if let Some(error) = &self.error {
            return Err(Error::llm(error.clone()));
        }

        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            panic!("MockLlmClient: no more responses configured");
        }

        Ok(responses.remove(0))
    }
}

// Helper functions to create mock responses
fn create_direct_response(content: &str) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1234567890,
        model: "gpt-4".to_string(),
        choices: vec![ChatCompletionChoice {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            finish_reason: Some("stop".to_string()),
            index: 0,
        }],
        usage: None,
    }
}

fn create_tool_request_response(tool_name: &str, tool_args: &str) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1234567890,
        model: "gpt-4".to_string(),
        choices: vec![ChatCompletionChoice {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: "".to_string(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_123".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: tool_name.to_string(),
                        arguments: tool_args.to_string(),
                    },
                }]),
                tool_call_id: None,
                name: None,
            },
            finish_reason: Some("tool_calls".to_string()),
            index: 0,
        }],
        usage: None,
    }
}

/// Test Agent::Process with direct LLM response (no tools)
/// This matches Go TestAgentProcess_LLMRespondsDirectly
#[tokio::test]
async fn test_agent_process_llm_responds_directly() {
    let llm_response = "Hello, I am a helpful AI.";
    let _llm_config = LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: None,
    };

    let mock_llm = MockLlmClient::new();
    mock_llm.add_response(create_direct_response(llm_response));

    // Create agent with no MCP servers (no tools)
    let mut agent = Agent::new_for_testing(
        Box::new(mock_llm),
        HashMap::new(),
        HashMap::new(),
        Vec::new(),
    );

    // Verify no tools available
    assert_eq!(agent.get_available_tools().len(), 0);

    // Create temporary history
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy())
        .await
        .unwrap();

    // Process request
    let result = agent
        .process("test-session", "User says hi", &history)
        .await
        .unwrap();

    assert_eq!(result, llm_response);
}

/// Test Agent::Process with successful tool execution
/// This matches Go TestAgentProcess_LLMRequestsMCPTool_Success
#[tokio::test]
async fn test_agent_process_llm_requests_mcp_tool_success() {
    let tool_name = "get_weather";
    let tool_args = r#"{"location": "London"}"#;
    let mcp_result = "The weather in London is sunny.";
    let final_response = "Based on the weather tool, it's sunny in London.";

    let _llm_config = LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: None,
    };

    let mock_llm = MockLlmClient::new();
    // First response: LLM requests tool
    mock_llm.add_response(create_tool_request_response(tool_name, tool_args));
    // Second response: LLM provides final answer
    mock_llm.add_response(create_direct_response(final_response));

    // Create tool definition
    let tools = vec![Tool {
        tool_type: "function".to_string(),
        function: LlmFunction {
            name: tool_name.to_string(),
            description: "Gets weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        },
    }];

    // Create mock MCP client
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert(tool_name.to_string(), "weather_client".to_string());

    let mock_mcp = MockMcpClient::new().with_tool_response(
        tool_name.to_string(),
        McpToolCallResponse {
            content: vec![McpContent::Text {
                text: mcp_result.to_string(),
            }],
            is_error: false,
        },
    );

    mcp_clients.insert("weather_client".to_string(), Box::new(mock_mcp));

    let mut agent =
        Agent::new_for_testing(Box::new(mock_llm), mcp_clients, tool_to_client_map, tools);

    // Create temporary history
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy())
        .await
        .unwrap();

    // Process request
    let result = agent
        .process("test-session", "What's the weather in London?", &history)
        .await
        .unwrap();

    assert_eq!(result, final_response);
}

/// Test Agent::Process when MCP tool call fails
/// This matches Go TestAgentProcess_LLMRequestsMCPTool_MCPClientFails
#[tokio::test]
async fn test_agent_process_llm_requests_mcp_tool_mcp_client_fails() {
    let tool_name = "broken_tool";
    let tool_args = r#"{}"#;
    let final_response =
        "MCP tool call failed for all configured servers or tool not found (FSM helper).";

    let _llm_config = LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: None,
    };

    let mock_llm = MockLlmClient::new();
    // First response: LLM requests tool
    mock_llm.add_response(create_tool_request_response(tool_name, tool_args));
    // Second response: LLM handles error
    mock_llm.add_response(create_direct_response(final_response));

    // Create tool definition
    let tools = vec![Tool {
        tool_type: "function".to_string(),
        function: LlmFunction {
            name: tool_name.to_string(),
            description: "A tool that is broken".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    }];

    // Create mock MCP client that will fail
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert(tool_name.to_string(), "broken_client".to_string());

    let mock_mcp = MockMcpClient::new().with_tool_error(
        tool_name.to_string(),
        "MCP tool execution failed badly.".to_string(),
    );

    mcp_clients.insert("broken_client".to_string(), Box::new(mock_mcp));

    let mut agent =
        Agent::new_for_testing(Box::new(mock_llm), mcp_clients, tool_to_client_map, tools);

    // Create temporary history
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy())
        .await
        .unwrap();

    // Process request
    let result = agent
        .process("test-session", "Use the broken tool", &history)
        .await
        .unwrap();

    assert_eq!(result, final_response);
}

/// Test Agent::Process with sequential tool calls
/// This matches Go TestAgentProcess_SequentialToolCalls
#[tokio::test]
async fn test_agent_process_sequential_tool_calls() {
    let tool_a = "tool_A";
    let tool_a_args = r#"{"input_A": "value_A"}"#;
    let tool_a_result = "Result from Tool A";

    let tool_b = "tool_B";
    let tool_b_args = r#"{"input_B": "value_B_from_A_result"}"#;
    let tool_b_result = "Result from Tool B";

    let final_response = "After using Tool A and Tool B, the answer is complete.";

    let _llm_config = LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: None,
    };

    let mock_llm = MockLlmClient::new();
    // First response: LLM requests Tool A
    mock_llm.add_response(create_tool_request_response(tool_a, tool_a_args));
    // Second response: LLM requests Tool B
    mock_llm.add_response(create_tool_request_response(tool_b, tool_b_args));
    // Third response: LLM provides final answer
    mock_llm.add_response(create_direct_response(final_response));

    // Create tool definitions
    let tools = vec![
        Tool {
            tool_type: "function".to_string(),
            function: LlmFunction {
                name: tool_a.to_string(),
                description: "Tool A".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: LlmFunction {
                name: tool_b.to_string(),
                description: "Tool B".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        },
    ];

    // Create mock MCP clients
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert(tool_a.to_string(), "multi_client".to_string());
    tool_to_client_map.insert(tool_b.to_string(), "multi_client".to_string());

    let mock_mcp = MockMcpClient::new()
        .with_tool_response(
            tool_a.to_string(),
            McpToolCallResponse {
                content: vec![McpContent::Text {
                    text: tool_a_result.to_string(),
                }],
                is_error: false,
            },
        )
        .with_tool_response(
            tool_b.to_string(),
            McpToolCallResponse {
                content: vec![McpContent::Text {
                    text: tool_b_result.to_string(),
                }],
                is_error: false,
            },
        );

    mcp_clients.insert("multi_client".to_string(), Box::new(mock_mcp));

    let mut agent =
        Agent::new_for_testing(Box::new(mock_llm), mcp_clients, tool_to_client_map, tools);

    // Create temporary history
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy())
        .await
        .unwrap();

    // Process request
    let result = agent
        .process("test-session", "Sequential task", &history)
        .await
        .unwrap();

    assert_eq!(result, final_response);
}

/// Test Agent::Process when max turns exceeded
/// This matches Go TestAgentProcess_MaxTurnsExceeded
#[tokio::test]
async fn test_agent_process_max_turns_exceeded() {
    let tool_name = "looping_tool";
    let tool_args = r#"{}"#;

    let _llm_config = LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: None,
    };

    let mock_llm = MockLlmClient::new();
    // Create 6 responses - more than max turns (5)
    for _i in 0..6 {
        mock_llm.add_response(create_tool_request_response(tool_name, tool_args));
    }

    // Create tool definition
    let tools = vec![Tool {
        tool_type: "function".to_string(),
        function: LlmFunction {
            name: tool_name.to_string(),
            description: "A looping tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    }];

    // Create mock MCP client
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert(tool_name.to_string(), "loop_client".to_string());

    let mock_mcp = MockMcpClient::new().with_tool_response(
        tool_name.to_string(),
        McpToolCallResponse {
            content: vec![McpContent::Text {
                text: "looping tool result".to_string(),
            }],
            is_error: false,
        },
    );

    mcp_clients.insert("loop_client".to_string(), Box::new(mock_mcp));

    let mut agent =
        Agent::new_for_testing(Box::new(mock_llm), mcp_clients, tool_to_client_map, tools);

    // Create temporary history
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy())
        .await
        .unwrap();

    // Process request - should fail with max turns exceeded
    let result = agent.process("test-session", "Start loop", &history).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("exceeded maximum interaction turns")
    );
}

// Mock MCP client for testing
use jarvis_rust::mcp::{
    McpClient, McpGetPromptRequest, McpInitializeRequest, McpInitializeResponse, McpPrompt, McpTool,
};

struct MockMcpClient {
    tool_responses: HashMap<String, McpToolCallResponse>,
    tool_errors: HashMap<String, String>,
    initialize_error: Option<String>,
}

impl MockMcpClient {
    fn new() -> Self {
        Self {
            tool_responses: HashMap::new(),
            tool_errors: HashMap::new(),
            initialize_error: None,
        }
    }

    fn with_tool_response(mut self, tool_name: String, response: McpToolCallResponse) -> Self {
        self.tool_responses.insert(tool_name, response);
        self
    }

    fn with_tool_error(mut self, tool_name: String, error: String) -> Self {
        self.tool_errors.insert(tool_name, error);
        self
    }

    fn with_initialize_error(mut self, error: String) -> Self {
        self.initialize_error = Some(error);
        self
    }
}

#[async_trait]
impl McpClient for MockMcpClient {
    async fn initialize(
        &mut self,
        _request: McpInitializeRequest,
    ) -> Result<McpInitializeResponse> {
        if let Some(error) = &self.initialize_error {
            return Err(Error::mcp(error.clone()));
        }

        Ok(McpInitializeResponse {
            protocol_version: "1.0".to_string(),
            capabilities: jarvis_rust::mcp::McpServerCapabilities {
                prompts: None,
                resources: None,
                tools: None,
            },
            server_info: Some(jarvis_rust::mcp::McpServerInfo {
                name: "mock".to_string(),
                version: "1.0".to_string(),
            }),
        })
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>> {
        Ok(vec![])
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> Result<McpToolCallResponse> {
        if let Some(error) = self.tool_errors.get(&request.name) {
            return Err(Error::mcp(error.clone()));
        }

        if let Some(response) = self.tool_responses.get(&request.name) {
            return Ok(response.clone());
        }

        Ok(McpToolCallResponse {
            content: vec![McpContent::Text {
                text: format!("mock default success for {}", request.name),
            }],
            is_error: false,
        })
    }

    async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        Ok(vec![])
    }

    async fn get_prompt(
        &self,
        _request: McpGetPromptRequest,
    ) -> Result<jarvis_rust::mcp::McpGetPromptResponse> {
        Ok(jarvis_rust::mcp::McpGetPromptResponse {
            description: "".to_string(),
            messages: vec![],
        })
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
