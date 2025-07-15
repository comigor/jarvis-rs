use jarvis_rust::{
    agent::fsm::{AgentContext, AgentEvent, AgentState, AgentStateMachine},
    llm::{ChatMessage, Function, Tool},
    mcp::{McpContent, McpToolCallRequest, McpToolCallResponse},
};
use pretty_assertions::assert_eq;
use std::collections::HashMap;

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
    assert_eq!(fsm.context.max_turns, 5);
}

#[tokio::test]
async fn test_valid_state_transitions() {
    let mut fsm = create_test_fsm();

    // Initial state: ReadyToCallLlm
    assert_eq!(*fsm.current_state(), AgentState::ReadyToCallLlm);

    // ProcessInput: ReadyToCallLlm -> AwaitingLlmResponse
    fsm.process_event(AgentEvent::ProcessInput, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::AwaitingLlmResponse);

    // LlmRespondedWithContent: AwaitingLlmResponse -> Done
    fsm.process_event(AgentEvent::LlmRespondedWithContent, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::Done);
    assert!(fsm.is_terminal());
}

#[tokio::test]
async fn test_tool_execution_flow() {
    let mut fsm = create_test_fsm();

    // ReadyToCallLlm -> AwaitingLlmResponse
    fsm.process_event(AgentEvent::ProcessInput, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::AwaitingLlmResponse);

    // AwaitingLlmResponse -> ExecutingTools
    fsm.process_event(AgentEvent::LlmRequestedTools, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::ExecutingTools);

    // ExecutingTools -> ReadyToCallLlm
    fsm.process_event(AgentEvent::ToolsExecutionCompleted, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::ReadyToCallLlm);
}

#[tokio::test]
async fn test_error_transitions() {
    let mut fsm = create_test_fsm();

    // Error from ReadyToCallLlm
    fsm.process_event(AgentEvent::ErrorOccurred, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::Error);
    assert!(fsm.is_terminal());

    // Reset for next test
    let mut fsm = create_test_fsm();
    fsm.process_event(AgentEvent::ProcessInput, None)
        .await
        .unwrap();

    // Error from AwaitingLlmResponse
    fsm.process_event(AgentEvent::ErrorOccurred, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::Error);
    assert!(fsm.is_terminal());

    // Reset for next test
    let mut fsm = create_test_fsm();
    fsm.process_event(AgentEvent::ProcessInput, None)
        .await
        .unwrap();
    fsm.process_event(AgentEvent::LlmRequestedTools, None)
        .await
        .unwrap();

    // Error from ExecutingTools
    fsm.process_event(AgentEvent::ErrorOccurred, None)
        .await
        .unwrap();
    assert_eq!(*fsm.current_state(), AgentState::Error);
    assert!(fsm.is_terminal());
}

#[tokio::test]
async fn test_invalid_transitions() {
    let mut fsm = create_test_fsm();

    // Invalid: LlmRespondedWithContent from ReadyToCallLlm
    let result = fsm
        .process_event(AgentEvent::LlmRespondedWithContent, None)
        .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid transition")
    );

    // Invalid: ToolsExecutionCompleted from ReadyToCallLlm
    let result = fsm
        .process_event(AgentEvent::ToolsExecutionCompleted, None)
        .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid transition")
    );

    // Move to AwaitingLlmResponse
    fsm.process_event(AgentEvent::ProcessInput, None)
        .await
        .unwrap();

    // Invalid: ProcessInput from AwaitingLlmResponse
    let result = fsm.process_event(AgentEvent::ProcessInput, None).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid transition")
    );
}

#[test]
fn test_agent_context_methods() {
    let mut context = AgentContext::new(vec![], vec![], HashMap::new());

    // Test initial state
    assert_eq!(context.current_turn, 0);
    assert_eq!(context.max_turns, 5);
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
    for _ in 0..5 {
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
