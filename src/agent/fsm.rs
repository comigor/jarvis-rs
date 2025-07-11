use crate::{
    llm::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Tool},
    mcp::{McpClient, McpToolCallRequest, McpToolCallResponse},
    Error, Result,
};
use rust_fsm::*;
use std::collections::HashMap;
use tracing::{debug, error, warn};

// FSM States
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentState {
    ReadyToCallLlm,
    AwaitingLlmResponse,
    ExecutingTools,
    Done,
    Error,
}

// FSM Events
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentEvent {
    ProcessInput,
    LlmRespondedWithContent,
    LlmRequestedTools,
    ToolsExecutionCompleted,
    ToolsExecutionFailed,
    ErrorOccurred,
}

// Context for FSM operations
#[derive(Debug)]
pub struct AgentContext {
    pub messages: Vec<ChatMessage>,
    pub llm_response: Option<ChatCompletionResponse>,
    pub tool_calls: Vec<crate::llm::ToolCall>,
    pub tool_results: Vec<ChatMessage>,
    pub final_content: String,
    pub last_error: Option<Error>,
    pub current_turn: usize,
    pub max_turns: usize,
    pub available_tools: Vec<Tool>,
    pub mcp_clients: HashMap<String, String>, // Tool name -> MCP client name mapping
}

impl AgentContext {
    pub fn new(
        initial_messages: Vec<ChatMessage>,
        available_tools: Vec<Tool>,
        mcp_clients: HashMap<String, String>,
    ) -> Self {
        Self {
            messages: initial_messages,
            llm_response: None,
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            final_content: String::new(),
            last_error: None,
            current_turn: 0,
            max_turns: 5,
            available_tools,
            mcp_clients,
        }
    }

    pub fn add_error(&mut self, error: Error) {
        self.last_error = Some(error);
    }

    pub fn add_tool_result(&mut self, result: ChatMessage) {
        self.tool_results.push(result);
    }

    pub fn increment_turn(&mut self) {
        self.current_turn += 1;
    }

    pub fn is_max_turns_reached(&self) -> bool {
        self.current_turn >= self.max_turns
    }
}

// Define the FSM
state_machine! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[state_machine(
        state(crate::agent::fsm::AgentState),
        input(crate::agent::fsm::AgentEvent)
    )]
    pub AgentFsm(AgentState::ReadyToCallLlm)

    AgentState::ReadyToCallLlm(AgentEvent::ProcessInput) => AgentState::AwaitingLlmResponse,
    AgentState::AwaitingLlmResponse(AgentEvent::LlmRespondedWithContent) => AgentState::Done,
    AgentState::AwaitingLlmResponse(AgentEvent::LlmRequestedTools) => AgentState::ExecutingTools,
    AgentState::AwaitingLlmResponse(AgentEvent::ErrorOccurred) => AgentState::Error,
    AgentState::ExecutingTools(AgentEvent::ToolsExecutionCompleted) => AgentState::ReadyToCallLlm,
    AgentState::ExecutingTools(AgentEvent::ToolsExecutionFailed) => AgentState::Error,
    AgentState::ExecutingTools(AgentEvent::ErrorOccurred) => AgentState::Error,
    AgentState::ReadyToCallLlm(AgentEvent::ErrorOccurred) => AgentState::Error,
}

pub struct AgentStateMachine {
    fsm: AgentFsm::StateMachine,
    pub context: AgentContext,
}

impl AgentStateMachine {
    pub fn new(
        initial_messages: Vec<ChatMessage>,
        available_tools: Vec<Tool>,
        mcp_clients: HashMap<String, String>,
    ) -> Self {
        Self {
            fsm: AgentFsm::StateMachine::new(),
            context: AgentContext::new(initial_messages, available_tools, mcp_clients),
        }
    }

    pub fn current_state(&self) -> AgentState {
        *self.fsm.state()
    }

    pub fn is_terminal(&self) -> bool {
        matches!(*self.fsm.state(), AgentState::Done | AgentState::Error)
    }

    pub fn get_final_content(&self) -> &str {
        &self.context.final_content
    }

    pub fn get_last_error(&self) -> Option<&Error> {
        self.context.last_error.as_ref()
    }

    pub async fn process_event(
        &mut self,
        event: AgentEvent,
        llm_client: &dyn crate::llm::LlmClient,
    ) -> Result<()> {
        debug!("Processing FSM event: {:?} in state: {:?}", event, *self.fsm.state());

        // Handle state-specific logic before transition
        match (*self.fsm.state(), &event) {
            (AgentState::ReadyToCallLlm, AgentEvent::ProcessInput) => {
                self.handle_ready_to_call_llm(llm_client).await?;
            }
            (AgentState::ExecutingTools, AgentEvent::ToolsExecutionCompleted) => {
                self.handle_tools_execution_completed().await?;
            }
            _ => {}
        }

        // Attempt state transition
        match self.fsm.consume(&event) {
            Ok(_) => {
                debug!("FSM transitioned to state: {:?}", *self.fsm.state());
            }
            Err(_) => {
                let error = Error::InvalidTransition {
                    current: format!("{:?}", *self.fsm.state()),
                    requested: format!("{:?}", event),
                };
                self.context.add_error(error.clone());
                return Err(error);
            }
        }

        // Handle post-transition logic
        match *self.fsm.state() {
            AgentState::Done => {
                self.handle_done_state()?;
            }
            AgentState::Error => {
                self.handle_error_state()?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_ready_to_call_llm(
        &mut self,
        llm_client: &dyn crate::llm::LlmClient,
    ) -> Result<()> {
        if self.context.is_max_turns_reached() {
            let error = Error::MaxTurnsExceeded {
                max_turns: self.context.max_turns,
            };
            self.context.add_error(error.clone());
            return Err(error);
        }

        self.context.increment_turn();
        debug!("FSM: Making LLM call (turn {})", self.context.current_turn);

        let request = ChatCompletionRequest {
            model: "".to_string(), // This will be set by the LLM client
            messages: self.context.messages.clone(),
            tools: self.context.available_tools.clone(),
            max_tokens: None,
            temperature: Some(0.7),
        };

        match llm_client.create_chat_completion(request).await {
            Ok(response) => {
                debug!("LLM response received with {} choices", response.choices.len());
                self.context.llm_response = Some(response);
                
                // Determine next event based on response
                if let Some(ref response) = self.context.llm_response {
                    if !response.choices.is_empty() {
                        let choice = &response.choices[0];
                        if choice.message.tool_calls.is_some() && !choice.message.tool_calls.as_ref().unwrap().is_empty() {
                            self.context.tool_calls = choice.message.tool_calls.clone().unwrap();
                            // Will trigger LlmRequestedTools event
                        } else {
                            self.context.final_content = choice.message.content.clone();
                            // Will trigger LlmRespondedWithContent event
                        }
                    }
                }
            }
            Err(e) => {
                error!("LLM call failed: {}", e);
                self.context.add_error(e.clone());
                return Err(e);
            }
        }

        Ok(())
    }

    async fn handle_tools_execution_completed(&mut self) -> Result<()> {
        debug!("FSM: Tools execution completed, adding results to messages");
        
        // Add assistant message with tool calls to history
        if let Some(ref response) = self.context.llm_response {
            if !response.choices.is_empty() {
                let assistant_message = response.choices[0].message.clone();
                self.context.messages.push(assistant_message);
            }
        }

        // Add all tool results to messages
        self.context.messages.append(&mut self.context.tool_results);
        self.context.tool_results.clear();
        self.context.tool_calls.clear();

        Ok(())
    }

    fn handle_done_state(&mut self) -> Result<()> {
        debug!("FSM: Reached Done state");
        
        if let Some(ref response) = self.context.llm_response {
            if !response.choices.is_empty() && response.choices[0].message.tool_calls.is_none() {
                self.context.final_content = response.choices[0].message.content.clone();
            }
        }

        if self.context.final_content.is_empty() && self.context.last_error.is_none() {
            let error = Error::internal("FSM reached Done state without final content or error");
            self.context.add_error(error.clone());
            return Err(error);
        }

        Ok(())
    }

    fn handle_error_state(&mut self) -> Result<()> {
        debug!("FSM: Reached Error state");
        
        if self.context.last_error.is_none() {
            let error = Error::internal("FSM reached Error state without specific error");
            self.context.add_error(error);
        }

        Ok(())
    }

    pub fn prepare_tool_execution(&mut self) -> Vec<crate::llm::ToolCall> {
        debug!("FSM: Preparing to execute {} tools", self.context.tool_calls.len());

        if self.context.mcp_clients.is_empty() && !self.context.tool_calls.is_empty() {
            warn!("LLM requested tools, but no MCP clients are available");
            
            // Create error results for each tool call
            let tool_calls = self.context.tool_calls.clone();
            for tool_call in &tool_calls {
                let error_message = ChatMessage {
                    role: "tool".to_string(),
                    content: format!("Error: No MCP clients available to execute tool {}", tool_call.function.name),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.function.name.clone()),
                };
                self.context.add_tool_result(error_message);
            }
            
            return Vec::new();
        }

        self.context.tool_calls.clone()
    }

    pub fn add_tool_execution_results(&mut self, results: Vec<(String, String)>) {
        // results is Vec<(tool_call_id, content)>
        for (tool_call_id, content) in results {
            let result_message = ChatMessage {
                role: "tool".to_string(),
                content,
                tool_calls: None,
                tool_call_id: Some(tool_call_id),
                name: None,
            };
            
            self.context.add_tool_result(result_message);
        }
    }
}