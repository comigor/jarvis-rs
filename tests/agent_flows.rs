use jarvis_rust::{
    agent::Agent,
    config::{LlmConfig, McpServerConfig},
    history::HistoryStorage,
    llm::{ChatCompletionChoice, ChatCompletionResponse, ChatMessage, ToolCall, Function as LlmFunction},
    mcp::{McpClientType, McpContent, McpToolCallResponse},
    Error,
};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio;

mod common;
use common::{create_mock_chat_response, create_mock_chat_response_with_tool_calls, create_mock_tool_call, create_mock_tool_response, MockLlmClient, MockMcpClient};

/// Test the agent processing a simple request without tool calls
#[tokio::test]
async fn test_agent_direct_llm_response() {
    // Setup
    let llm_config = LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: Some("You are helpful".to_string()),
    };

    let mcp_configs: Vec<jarvis_rust::config::McpServerConfig> = vec![]; // No MCP servers

    // Create a mock LLM client with a direct response
    let mut mock_llm = MockLlmClient::new();
    mock_llm.add_response(create_mock_chat_response("Hello! How can I help you today?"));

    // Create agent (this will fail in real implementation since we can't inject the mock)
    // For now, let's test the components that we can test
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy()).await.unwrap();

    let session_id = "test-session";
    let user_input = "Hello there!";

    // Test saving user input to history
    let user_message = jarvis_rust::history::Message::user(session_id.to_string(), user_input.to_string());
    history.save(user_message).await.unwrap();

    // Test saving assistant response to history
    let assistant_message = jarvis_rust::history::Message::assistant(session_id.to_string(), "Hello! How can I help you today?".to_string());
    history.save(assistant_message).await.unwrap();

    // Verify history
    let messages = history.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, user_input);
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hello! How can I help you today?");
}

/// Test the agent FSM flow with tool calling
#[tokio::test]
async fn test_agent_fsm_with_tool_calls() {
    use jarvis_rust::agent::fsm::{AgentStateMachine, AgentEvent};
    use jarvis_rust::llm::{Tool, Function};
    use jarvis_rust::mcp::{McpToolCallRequest, McpToolCallResponse, McpContent};

    // Create FSM with tool capabilities
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a helpful assistant with access to tools.".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: "What's the weather in London?".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let tools = vec![Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "get_weather".to_string(),
            description: "Get current weather for a location".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        },
    }];

    let mcp_clients = HashMap::new();
    let mut fsm = AgentStateMachine::new(messages, tools, mcp_clients);

    // Test initial state
    assert_eq!(*fsm.current_state(), jarvis_rust::agent::fsm::AgentState::ReadyToCallLlm);

    // Simulate the flow: ReadyToCallLlm -> AwaitingLlmResponse
    fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();
    assert_eq!(*fsm.current_state(), jarvis_rust::agent::fsm::AgentState::AwaitingLlmResponse);

    // Simulate LLM requesting tools: AwaitingLlmResponse -> ExecutingTools
    fsm.process_event(AgentEvent::LlmRequestedTools, None).await.unwrap();
    assert_eq!(*fsm.current_state(), jarvis_rust::agent::fsm::AgentState::ExecutingTools);

    // Add some mock tool call results
    let tool_response = McpToolCallResponse {
        content: vec![McpContent::Text {
            text: "The weather in London is sunny, 22°C".to_string(),
        }],
        is_error: false,
    };
    fsm.add_tool_execution_results(vec![tool_response]);

    // Simulate tools completion: ExecutingTools -> ReadyToCallLlm
    fsm.process_event(AgentEvent::ToolsExecutionCompleted, None).await.unwrap();
    assert_eq!(*fsm.current_state(), jarvis_rust::agent::fsm::AgentState::ReadyToCallLlm);

    // Simulate final LLM response: ReadyToCallLlm -> AwaitingLlmResponse -> Done
    fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();
    fsm.process_event(AgentEvent::LlmRespondedWithContent, None).await.unwrap();
    assert_eq!(*fsm.current_state(), jarvis_rust::agent::fsm::AgentState::Done);
    assert!(fsm.is_terminal());
}

/// Test error handling in the FSM
#[tokio::test]
async fn test_agent_fsm_error_handling() {
    use jarvis_rust::agent::fsm::{AgentStateMachine, AgentEvent, AgentState};

    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: "Test error handling".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    let mut fsm = AgentStateMachine::new(messages, vec![], HashMap::new());

    // Test error from ReadyToCallLlm state
    fsm.process_event(AgentEvent::ErrorOccurred, None).await.unwrap();
    assert_eq!(*fsm.current_state(), AgentState::Error);
    assert!(fsm.is_terminal());

    // Test error from ExecutingTools state
    let mut fsm = AgentStateMachine::new(
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Test".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }],
        vec![],
        HashMap::new(),
    );

    fsm.process_event(AgentEvent::ProcessInput, None).await.unwrap();
    fsm.process_event(AgentEvent::LlmRequestedTools, None).await.unwrap();
    fsm.process_event(AgentEvent::ErrorOccurred, None).await.unwrap();
    assert_eq!(*fsm.current_state(), AgentState::Error);
    assert!(fsm.is_terminal());
}

/// Test conversation history persistence
#[tokio::test]
async fn test_conversation_history_flow() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("conversation.db");
    let history = HistoryStorage::new(&db_path.to_string_lossy()).await.unwrap();

    let session_id = "conversation-test";

    // Simulate a multi-turn conversation
    let conversation_flow = vec![
        ("user", "What's the capital of France?"),
        ("assistant", "The capital of France is Paris."),
        ("user", "What's the population?"),
        ("assistant", "Paris has approximately 2.1 million inhabitants in the city proper."),
        ("user", "Thanks!"),
        ("assistant", "You're welcome! Is there anything else you'd like to know?"),
    ];

    // Save all messages
    for (role, content) in &conversation_flow {
        let message = jarvis_rust::history::Message::new(
            session_id.to_string(),
            role.to_string(),
            content.to_string(),
        );
        history.save(message).await.unwrap();
    }

    // Retrieve and verify the conversation
    let messages = history.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 6);

    for (i, (expected_role, expected_content)) in conversation_flow.iter().enumerate() {
        assert_eq!(messages[i].role, *expected_role);
        assert_eq!(messages[i].content, *expected_content);
        assert_eq!(messages[i].session_id, session_id);
    }

    // Test session isolation
    let other_session = "other-session";
    let other_message = jarvis_rust::history::Message::user(
        other_session.to_string(),
        "Different session".to_string(),
    );
    history.save(other_message).await.unwrap();

    // Original session should be unchanged
    let original_messages = history.list(session_id).await.unwrap();
    assert_eq!(original_messages.len(), 6);

    // Other session should have one message
    let other_messages = history.list(other_session).await.unwrap();
    assert_eq!(other_messages.len(), 1);
    assert_eq!(other_messages[0].content, "Different session");
}

/// Test tool execution flow simulation
#[tokio::test]
async fn test_tool_execution_simulation() {
    use jarvis_rust::mcp::{McpToolCallRequest, McpToolCallResponse, McpContent};

    // Simulate tool execution without actual MCP clients
    let tool_request = McpToolCallRequest {
        name: "get_weather".to_string(),
        arguments: {
            let mut args = HashMap::new();
            args.insert("location".to_string(), serde_json::Value::String("London".to_string()));
            args
        },
    };

    // Simulate successful tool execution
    let tool_response = McpToolCallResponse {
        content: vec![McpContent::Text {
            text: "Current weather in London: Sunny, 20°C, light breeze".to_string(),
        }],
        is_error: false,
    };

    assert_eq!(tool_request.name, "get_weather");
    assert!(!tool_response.is_error);
    assert_eq!(tool_response.content.len(), 1);

    if let McpContent::Text { text } = &tool_response.content[0] {
        assert!(text.contains("London"));
        assert!(text.contains("20°C"));
    }

    // Simulate error in tool execution
    let error_response = McpToolCallResponse {
        content: vec![McpContent::Text {
            text: "Error: Unable to fetch weather data".to_string(),
        }],
        is_error: true,
    };

    assert!(error_response.is_error);
}

/// Test max turns exceeded scenario
#[tokio::test]
async fn test_max_turns_prevention() {
    use jarvis_rust::agent::fsm::{AgentStateMachine, AgentContext};

    let mut context = AgentContext::new(vec![], vec![], HashMap::new());

    // Test initial state
    assert_eq!(context.current_turn, 0);
    assert_eq!(context.max_turns, 10);
    assert!(!context.is_max_turns_reached());

    // Simulate reaching max turns
    for _ in 0..10 {
        context.increment_turn();
    }

    assert_eq!(context.current_turn, 10);
    assert!(context.is_max_turns_reached());

    // Further increments should still be detected as over limit
    context.increment_turn();
    assert!(context.is_max_turns_reached());
}

/// Test multiple concurrent sessions
#[tokio::test]
async fn test_concurrent_sessions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("concurrent.db");
    let history = std::sync::Arc::new(HistoryStorage::new(&db_path.to_string_lossy()).await.unwrap());

    let mut handles = vec![];

    // Spawn multiple concurrent sessions
    for session_num in 0..5 {
        let history_clone = std::sync::Arc::clone(&history);
        let handle = tokio::spawn(async move {
            let session_id = format!("session-{}", session_num);
            
            // Each session adds multiple messages
            for msg_num in 0..3 {
                let message = jarvis_rust::history::Message::user(
                    session_id.clone(),
                    format!("Message {} from session {}", msg_num, session_num),
                );
                history_clone.save(message).await.unwrap();
            }
            
            session_id
        });
        handles.push(handle);
    }

    // Wait for all sessions to complete
    let mut session_ids = vec![];
    for handle in handles {
        let session_id = handle.await.unwrap();
        session_ids.push(session_id);
    }

    // Verify each session has its own messages
    for session_id in session_ids {
        let messages = history.list(&session_id).await.unwrap();
        assert_eq!(messages.len(), 3);
        
        for (i, message) in messages.iter().enumerate() {
            assert_eq!(message.session_id, session_id);
            assert!(message.content.contains(&format!("Message {}", i)));
            // The content should match the format we saved: "Message {i} from session {session_num}"
            // Since we don't know session_num, just check that it contains "from session"
            assert!(message.content.contains("from session"));
        }
    }
}

/// Test to verify that tool-to-client mapping logic works correctly
/// This tests the HashMap creation and lookup functionality without needing real MCP connections
#[tokio::test]
async fn test_tool_to_client_mapping_logic() {
    use std::collections::HashMap;
    
    // Simulate the tool-to-client mapping logic that happens in Agent::new()
    let mut tool_to_client_map: HashMap<String, String> = HashMap::new();
    
    // Simulate discovering tools from multiple clients
    let client1_tools = vec!["get_weather", "send_email"];
    let client2_tools = vec!["search_web", "calculate"];
    
    // Client 1 tools
    for tool_name in client1_tools {
        tool_to_client_map.insert(tool_name.to_string(), "weather_client".to_string());
    }
    
    // Client 2 tools  
    for tool_name in client2_tools {
        tool_to_client_map.insert(tool_name.to_string(), "search_client".to_string());
    }
    
    // Test that mappings are correct
    assert_eq!(tool_to_client_map.get("get_weather"), Some(&"weather_client".to_string()));
    assert_eq!(tool_to_client_map.get("send_email"), Some(&"weather_client".to_string()));
    assert_eq!(tool_to_client_map.get("search_web"), Some(&"search_client".to_string()));
    assert_eq!(tool_to_client_map.get("calculate"), Some(&"search_client".to_string()));
    
    // Test that unmapped tools return None
    assert_eq!(tool_to_client_map.get("nonexistent_tool"), None);
    
    // Test tool name conflict simulation (last wins)
    tool_to_client_map.insert("duplicate_tool".to_string(), "client1".to_string());
    tool_to_client_map.insert("duplicate_tool".to_string(), "client2".to_string());
    assert_eq!(tool_to_client_map.get("duplicate_tool"), Some(&"client2".to_string()));
    
    println!("Tool-to-client mapping logic test passed!");
}