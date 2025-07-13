use async_trait::async_trait;
use jarvis_rust::{
    Error, Result,
    llm::{
        ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, FunctionCall,
        LlmClient, ToolCall,
    },
    mcp::{
        McpClient, McpContent, McpGetPromptRequest, McpGetPromptResponse, McpInitializeRequest,
        McpInitializeResponse, McpPrompt, McpPromptMessage, McpPromptsCapability,
        McpServerCapabilities, McpServerInfo, McpTool, McpToolCallRequest, McpToolCallResponse,
        McpToolsCapability,
    },
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock LLM client for testing
#[derive(Debug)]
pub struct MockLlmClient {
    pub responses: Arc<Mutex<Vec<ChatCompletionResponse>>>,
    pub requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
    pub error: Option<String>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(Vec::new())),
            error: None,
        }
    }

    pub fn with_responses(self, responses: Vec<ChatCompletionResponse>) -> Self {
        *self.responses.lock().unwrap() = responses;
        self
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    pub fn add_response(&self, response: ChatCompletionResponse) {
        self.responses.lock().unwrap().push(response);
    }

    pub fn get_requests(&self) -> Vec<ChatCompletionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn create_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        self.requests.lock().unwrap().push(request);

        if let Some(ref error) = self.error {
            return Err(Error::llm(error.clone()));
        }

        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(Error::llm("No more mock responses available"));
        }

        Ok(responses.remove(0))
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock MCP client for testing
#[derive(Debug)]
pub struct MockMcpClient {
    pub tools: Arc<Mutex<Vec<McpTool>>>,
    pub prompts: Arc<Mutex<Vec<McpPrompt>>>,
    pub tool_responses: Arc<Mutex<HashMap<String, McpToolCallResponse>>>,
    pub tool_errors: Arc<Mutex<HashMap<String, String>>>,
    pub initialize_error: Option<String>,
}

impl MockMcpClient {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(Mutex::new(Vec::new())),
            prompts: Arc::new(Mutex::new(Vec::new())),
            tool_responses: Arc::new(Mutex::new(HashMap::new())),
            tool_errors: Arc::new(Mutex::new(HashMap::new())),
            initialize_error: None,
        }
    }

    pub fn with_tools(self, tools: Vec<McpTool>) -> Self {
        *self.tools.lock().unwrap() = tools;
        self
    }

    pub fn with_tool_response(self, tool_name: String, response: McpToolCallResponse) -> Self {
        self.tool_responses
            .lock()
            .unwrap()
            .insert(tool_name, response);
        self
    }

    pub fn with_tool_error(self, tool_name: String, error: String) -> Self {
        self.tool_errors.lock().unwrap().insert(tool_name, error);
        self
    }

    pub fn with_initialize_error(mut self, error: String) -> Self {
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
        if let Some(ref error) = self.initialize_error {
            return Err(Error::mcp(error.clone()));
        }

        let tools = self.tools.lock().unwrap();
        let prompts = self.prompts.lock().unwrap();
        Ok(McpInitializeResponse {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpServerCapabilities {
                prompts: if prompts.is_empty() {
                    None
                } else {
                    Some(McpPromptsCapability {
                        list_changed: false,
                    })
                },
                resources: None,
                tools: if tools.is_empty() {
                    None
                } else {
                    Some(McpToolsCapability {
                        list_changed: false,
                    })
                },
            },
            server_info: Some(McpServerInfo {
                name: "mock-server".to_string(),
                version: "1.0.0".to_string(),
            }),
        })
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>> {
        Ok(self.tools.lock().unwrap().clone())
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> Result<McpToolCallResponse> {
        let tool_errors = self.tool_errors.lock().unwrap();
        if let Some(error) = tool_errors.get(&request.name) {
            return Err(Error::mcp(error.clone()));
        }
        drop(tool_errors);

        let tool_responses = self.tool_responses.lock().unwrap();
        if let Some(response) = tool_responses.get(&request.name) {
            return Ok(response.clone());
        }
        drop(tool_responses);

        // Default response
        Ok(McpToolCallResponse {
            content: vec![McpContent::Text {
                text: format!("Mock response for tool: {}", request.name),
            }],
            is_error: false,
        })
    }

    async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        Ok(self.prompts.lock().unwrap().clone())
    }

    async fn get_prompt(&self, _request: McpGetPromptRequest) -> Result<McpGetPromptResponse> {
        Ok(McpGetPromptResponse {
            description: "Mock prompt".to_string(),
            messages: vec![McpPromptMessage {
                role: "assistant".to_string(),
                content: McpContent::Text {
                    text: "Mock prompt content".to_string(),
                },
            }],
        })
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Default for MockMcpClient {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for creating test data

pub fn create_mock_chat_response(content: &str) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: "test-id".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "test-model".to_string(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: None,
    }
}

pub fn create_mock_chat_response_with_tool_calls(
    content: &str,
    tool_calls: Vec<ToolCall>,
) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: "test-id".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "test-model".to_string(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
                tool_calls: Some(tool_calls),
                tool_call_id: None,
                name: None,
            },
            finish_reason: Some("tool_calls".to_string()),
        }],
        usage: None,
    }
}

pub fn create_mock_tool_call(id: &str, function_name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: function_name.to_string(),
            arguments: arguments.to_string(),
        },
    }
}

pub fn create_mock_mcp_tool(name: &str, description: &str) -> McpTool {
    McpTool {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string"
                }
            }
        }),
    }
}

pub fn create_mock_tool_response(content: &str) -> McpToolCallResponse {
    McpToolCallResponse {
        content: vec![McpContent::Text {
            text: content.to_string(),
        }],
        is_error: false,
    }
}

pub fn create_mock_tool_error_response(error: &str) -> McpToolCallResponse {
    McpToolCallResponse {
        content: vec![McpContent::Text {
            text: error.to_string(),
        }],
        is_error: true,
    }
}
