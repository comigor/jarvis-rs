use crate::{
    llm::{ChatCompletionResponse, ChatMessage, Tool},
    mcp::{McpToolCallRequest, McpToolCallResponse},
    Error, Result,
};
use std::collections::HashMap;
use tracing::{debug, info, warn};

// Agent states
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    ReadyToCallLlm,
    AwaitingLlmResponse,
    ExecutingTools,
    Done,
    Error,
}

// Agent events
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    ProcessInput,
    LlmRespondedWithContent,
    LlmRequestedTools,
    ToolsExecutionCompleted,
    #[allow(dead_code)]
    ToolsExecutionFailed,
    #[allow(dead_code)]
    ErrorOccurred,
}

// Agent context (shared state)
#[derive(Debug, Clone)]
pub struct AgentContext {
    pub messages: Vec<ChatMessage>,
    #[allow(dead_code)]
    pub available_tools: Vec<Tool>,
    #[allow(dead_code)]
    pub mcp_clients: HashMap<String, String>,
    #[allow(dead_code)]
    pub current_turn: usize,
    #[allow(dead_code)]
    pub max_turns: usize,
    pub pending_tool_calls: Vec<McpToolCallRequest>,
    pub tool_call_results: Vec<McpToolCallResponse>,
    pub tool_call_id_mapping: Vec<String>, // Maps MCP tool call index to original LLM tool call ID
    pub last_error: Option<String>,
    pub llm_response: Option<ChatCompletionResponse>,
}

impl AgentContext {
    pub fn new(
        initial_messages: Vec<ChatMessage>,
        available_tools: Vec<Tool>,
        mcp_clients: HashMap<String, String>,
    ) -> Self {
        Self {
            messages: initial_messages,
            available_tools,
            mcp_clients,
            current_turn: 0,
            max_turns: 10, // Default limit
            pending_tool_calls: Vec::new(),
            tool_call_results: Vec::new(),
            tool_call_id_mapping: Vec::new(),
            last_error: None,
            llm_response: None,
        }
    }

    #[allow(dead_code)]
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    #[allow(dead_code)]
    pub fn increment_turn(&mut self) {
        self.current_turn += 1;
    }

    #[allow(dead_code)]
    pub fn set_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    #[allow(dead_code)]
    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    #[allow(dead_code)]
    pub fn has_reached_max_turns(&self) -> bool {
        self.current_turn >= self.max_turns
    }

    #[allow(dead_code)]
    pub fn set_pending_tool_calls(&mut self, calls: Vec<McpToolCallRequest>) {
        self.pending_tool_calls = calls;
    }

    #[allow(dead_code)]
    pub fn add_tool_call_result(&mut self, result: McpToolCallResponse) {
        self.tool_call_results.push(result);
    }

    #[allow(dead_code)]
    pub fn clear_tool_calls(&mut self) {
        self.pending_tool_calls.clear();
        self.tool_call_results.clear();
        self.tool_call_id_mapping.clear();
    }

    #[allow(dead_code)]
    pub fn is_max_turns_reached(&self) -> bool {
        self.current_turn >= self.max_turns
    }
}

// Simple FSM implementation
pub struct AgentStateMachine {
    state: AgentState,
    pub context: AgentContext,
}

impl AgentStateMachine {
    pub fn new(
        initial_messages: Vec<ChatMessage>,
        available_tools: Vec<Tool>,
        mcp_clients: HashMap<String, String>,
    ) -> Self {
        info!(
            "🚀 Creating new FSM with {} messages, {} tools, {} MCP clients",
            initial_messages.len(),
            available_tools.len(),
            mcp_clients.len()
        );
        Self {
            state: AgentState::ReadyToCallLlm,
            context: AgentContext::new(initial_messages, available_tools, mcp_clients),
        }
    }

    pub fn current_state(&self) -> &AgentState {
        &self.state
    }

    pub fn transition(&mut self, event: AgentEvent) -> Result<()> {
        let old_state = self.state.clone();
        debug!(
            "🔄 FSM processing event {:?} in state {:?}",
            event, old_state
        );

        let new_state = match (&self.state, &event) {
            (AgentState::ReadyToCallLlm, AgentEvent::ProcessInput) => {
                AgentState::AwaitingLlmResponse
            }
            (AgentState::AwaitingLlmResponse, AgentEvent::LlmRespondedWithContent) => {
                AgentState::Done
            }
            (AgentState::AwaitingLlmResponse, AgentEvent::LlmRequestedTools) => {
                AgentState::ExecutingTools
            }
            (AgentState::AwaitingLlmResponse, AgentEvent::ErrorOccurred) => AgentState::Error,
            (AgentState::ExecutingTools, AgentEvent::ToolsExecutionCompleted) => {
                AgentState::ReadyToCallLlm
            }
            (AgentState::ExecutingTools, AgentEvent::ToolsExecutionFailed) => AgentState::Error,
            (AgentState::ExecutingTools, AgentEvent::ErrorOccurred) => AgentState::Error,
            (AgentState::ReadyToCallLlm, AgentEvent::ErrorOccurred) => AgentState::Error,
            _ => {
                warn!(
                    "❌ Invalid FSM transition from {:?} with event {:?}",
                    self.state, event
                );
                return Err(Error::fsm(format!(
                    "Invalid transition from {:?} with event {:?}",
                    self.state, event
                )));
            }
        };

        if old_state != new_state {
            info!(
                "🎯 FSM state transition: {:?} -> {:?} (event: {:?})",
                old_state, new_state, event
            );
        } else {
            debug!(
                "🔄 FSM staying in state {:?} after event {:?}",
                old_state, event
            );
        }

        self.state = new_state;
        Ok(())
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.state, AgentState::Done | AgentState::Error)
    }

    pub async fn process_event(
        &mut self,
        event: AgentEvent,
        _llm_client: Option<&dyn crate::llm::LlmClient>,
    ) -> Result<()> {
        debug!(
            "📨 FSM received event {:?} in state {:?}",
            event, self.state
        );
        self.transition(event)
    }

    pub fn prepare_tool_execution(&mut self) -> Vec<McpToolCallRequest> {
        self.context.pending_tool_calls.clone()
    }

    pub fn add_tool_execution_results(&mut self, results: Vec<McpToolCallResponse>) {
        self.context.tool_call_results.extend(results);
    }

    pub fn get_final_content(&self) -> &str {
        if let Some(last_message) = self.context.messages.last() {
            &last_message.content
        } else {
            ""
        }
    }

    pub fn get_last_error(&self) -> Option<&str> {
        self.context.last_error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use crate::llm::{ChatMessage, Tool, Function};
    use crate::mcp::McpContent;

    fn create_test_fsm() -> AgentStateMachine {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Test message".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        
        let tools = vec![Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                parameters: serde_json::json!({}),
            },
        }];
        
        let mcp_clients = HashMap::new();
        
        AgentStateMachine::new(messages, tools, mcp_clients)
    }

    #[test]
    fn test_fsm_initial_state() {
        let fsm = create_test_fsm();
        assert_eq!(*fsm.current_state(), AgentState::ReadyToCallLlm);
        assert!(!fsm.is_terminal());
        assert_eq!(fsm.context.messages.len(), 1);
        assert_eq!(fsm.context.available_tools.len(), 1);
        assert_eq!(fsm.context.current_turn, 0);
        assert_eq!(fsm.context.max_turns, 10);
    }

    #[tokio::test]
    async fn test_valid_state_transitions() {
        let mut fsm = create_test_fsm();

        // Initial state: ReadyToCallLlm
        assert_eq!(*fsm.current_state(), AgentState::ReadyToCallLlm);

        // ProcessInput: ReadyToCallLlm -> AwaitingLlmResponse
        fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::AwaitingLlmResponse);

        // LlmRespondedWithContent: AwaitingLlmResponse -> Done
        fsm.process_event(AgentEvent::LlmRespondedWithContent, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::Done);
        assert!(fsm.is_terminal());
    }

    #[tokio::test]
    async fn test_tool_execution_flow() {
        let mut fsm = create_test_fsm();

        // ReadyToCallLlm -> AwaitingLlmResponse
        fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::AwaitingLlmResponse);

        // AwaitingLlmResponse -> ExecutingTools
        fsm.process_event(AgentEvent::LlmRequestedTools, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::ExecutingTools);

        // ExecutingTools -> ReadyToCallLlm
        fsm.process_event(AgentEvent::ToolsExecutionCompleted, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::ReadyToCallLlm);
    }

    #[tokio::test]
    async fn test_error_transitions() {
        let mut fsm = create_test_fsm();

        // Error from ReadyToCallLlm
        fsm.process_event(AgentEvent::ErrorOccurred, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::Error);
        assert!(fsm.is_terminal());

        // Reset for next test
        let mut fsm = create_test_fsm();
        fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();

        // Error from AwaitingLlmResponse
        fsm.process_event(AgentEvent::ErrorOccurred, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::Error);
        assert!(fsm.is_terminal());

        // Reset for next test
        let mut fsm = create_test_fsm();
        fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();
        fsm.process_event(AgentEvent::LlmRequestedTools, None).await.unwrap();

        // Error from ExecutingTools
        fsm.process_event(AgentEvent::ErrorOccurred, None).await.unwrap();
        assert_eq!(*fsm.current_state(), AgentState::Error);
        assert!(fsm.is_terminal());
    }

    #[tokio::test]
    async fn test_invalid_transitions() {
        let mut fsm = create_test_fsm();

        // Invalid: LlmRespondedWithContent from ReadyToCallLlm
        let result = fsm.process_event(AgentEvent::LlmRespondedWithContent, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid transition"));

        // Invalid: ToolsExecutionCompleted from ReadyToCallLlm
        let result = fsm.process_event(AgentEvent::ToolsExecutionCompleted, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid transition"));

        // Move to AwaitingLlmResponse
        fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();

        // Invalid: ProcessInput from AwaitingLlmResponse
        let result = fsm.process_event(AgentEvent::ProcessInput, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid transition"));
    }

    #[test]
    fn test_agent_context_methods() {
        let mut context = AgentContext::new(vec![], vec![], HashMap::new());

        // Test initial state
        assert_eq!(context.current_turn, 0);
        assert_eq!(context.max_turns, 10);
        assert!(context.last_error.is_none());
        assert!(!context.has_reached_max_turns());

        // Test message addition
        let message = ChatMessage {
            role: "user".to_string(),
            content: "Test".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        context.add_message(message.clone());
        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.messages[0].content, "Test");

        // Test turn increment
        context.increment_turn();
        assert_eq!(context.current_turn, 1);

        // Test error handling
        context.set_error("Test error".to_string());
        assert_eq!(context.last_error, Some("Test error".to_string()));
        context.clear_error();
        assert!(context.last_error.is_none());

        // Test max turns
        for _ in 0..10 {
            context.increment_turn();
        }
        assert!(context.is_max_turns_reached());
    }

    #[test]
    fn test_tool_call_management() {
        let mut context = AgentContext::new(vec![], vec![], HashMap::new());

        // Test pending tool calls
        let tool_calls = vec![
            McpToolCallRequest {
                name: "tool1".to_string(),
                arguments: HashMap::new(),
            },
            McpToolCallRequest {
                name: "tool2".to_string(),
                arguments: HashMap::new(),
            },
        ];

        context.set_pending_tool_calls(tool_calls.clone());
        assert_eq!(context.pending_tool_calls.len(), 2);

        // Test tool call results
        let result = McpToolCallResponse {
            content: vec![McpContent::Text {
                text: "Tool result".to_string(),
            }],
            is_error: false,
        };

        context.add_tool_call_result(result.clone());
        assert_eq!(context.tool_call_results.len(), 1);

        // Test clearing tool calls
        context.clear_tool_calls();
        assert!(context.pending_tool_calls.is_empty());
        assert!(context.tool_call_results.is_empty());
        assert!(context.tool_call_id_mapping.is_empty());
    }

    #[test]
    fn test_fsm_tool_execution_methods() {
        let mut fsm = create_test_fsm();

        // Test prepare tool execution with no tool calls
        let tool_calls = fsm.prepare_tool_execution();
        assert!(tool_calls.is_empty());

        // Add some tool calls to context
        let test_tool_calls = vec![McpToolCallRequest {
            name: "test_tool".to_string(),
            arguments: HashMap::new(),
        }];
        fsm.context.pending_tool_calls = test_tool_calls.clone();

        let prepared_calls = fsm.prepare_tool_execution();
        assert_eq!(prepared_calls.len(), 1);
        assert_eq!(prepared_calls[0].name, "test_tool");

        // Test adding tool execution results
        let results = vec![McpToolCallResponse {
            content: vec![McpContent::Text {
                text: "Tool executed successfully".to_string(),
            }],
            is_error: false,
        }];

        fsm.add_tool_execution_results(results);
        assert_eq!(fsm.context.tool_call_results.len(), 1);
    }

    #[test]
    fn test_final_content_retrieval() {
        let mut fsm = create_test_fsm();

        // Test with initial message
        assert_eq!(fsm.get_final_content(), "Test message");

        // Add more messages
        fsm.context.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: "Assistant response".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        assert_eq!(fsm.get_final_content(), "Assistant response");

        // Test with empty messages
        fsm.context.messages.clear();
        assert_eq!(fsm.get_final_content(), "");
    }

    #[test]
    fn test_error_handling() {
        let mut fsm = create_test_fsm();

        // Initially no error
        assert!(fsm.get_last_error().is_none());

        // Set an error
        fsm.context.set_error("Test error occurred".to_string());
        assert_eq!(fsm.get_last_error(), Some("Test error occurred"));

        // Clear error
        fsm.context.clear_error();
        assert!(fsm.get_last_error().is_none());
    }

    #[test]
    fn test_fsm_creation_with_parameters() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "System prompt".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: "User input".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let tools = vec![
            Tool {
                tool_type: "function".to_string(),
                function: Function {
                    name: "weather".to_string(),
                    description: "Get weather".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            },
            Tool {
                tool_type: "function".to_string(),
                function: Function {
                    name: "calendar".to_string(),
                    description: "Calendar access".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            },
        ];

        let mut mcp_clients = HashMap::new();
        mcp_clients.insert("weather_server".to_string(), "url".to_string());

        let fsm = AgentStateMachine::new(messages, tools, mcp_clients);

        assert_eq!(fsm.context.messages.len(), 2);
        assert_eq!(fsm.context.available_tools.len(), 2);
        assert_eq!(fsm.context.mcp_clients.len(), 1);
        assert_eq!(*fsm.current_state(), AgentState::ReadyToCallLlm);
    }
}
