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
