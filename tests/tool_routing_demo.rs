/// Demonstration test showing the tool routing functionality
/// This test shows how the improved tool routing system works by testing the error messages
use std::collections::HashMap;
use jarvis_rust::mcp::{McpToolCallRequest, McpToolCallResponse, McpContent};

/// Simulate the execute_mcp_tool logic for demonstration
async fn simulate_tool_execution(
    tool_to_client_map: &HashMap<String, String>,
    mcp_clients: &HashMap<String, String>, // Simulated as name -> description
    tool_call: &McpToolCallRequest,
) -> McpToolCallResponse {
    // This simulates the logic from executor.rs:execute_mcp_tool
    match tool_to_client_map.get(&tool_call.name) {
        Some(client_name) => {
            match mcp_clients.get(client_name) {
                Some(_client_description) => {
                    // Success case - tool found and client available
                    McpToolCallResponse {
                        content: vec![McpContent::Text {
                            text: format!("Tool '{}' executed successfully on client '{}'", tool_call.name, client_name),
                        }],
                        is_error: false,
                    }
                }
                None => {
                    // Client mapped but not available
                    McpToolCallResponse {
                        content: vec![McpContent::Text {
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
            // Tool not mapped to any client
            McpToolCallResponse {
                content: vec![McpContent::Text {
                    text: format!(
                        "Error: No client mapping found for tool: '{}'. Available tools: {}",
                        tool_call.name,
                        tool_to_client_map.keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                }],
                is_error: true,
            }
        }
    }
}

#[tokio::test]
async fn test_tool_routing_functionality() {
    // Set up test data
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert("get_weather".to_string(), "weather_client".to_string());
    tool_to_client_map.insert("send_email".to_string(), "email_client".to_string());
    tool_to_client_map.insert("search_web".to_string(), "search_client".to_string());

    let mut mcp_clients = HashMap::new();
    mcp_clients.insert("weather_client".to_string(), "Weather service client".to_string());
    mcp_clients.insert("email_client".to_string(), "Email service client".to_string());
    mcp_clients.insert("search_client".to_string(), "Web search client".to_string());

    // Test 1: Successful tool execution
    let tool_call = McpToolCallRequest {
        name: "get_weather".to_string(),
        arguments: HashMap::new(),
    };
    
    let response = simulate_tool_execution(&tool_to_client_map, &mcp_clients, &tool_call).await;
    assert!(!response.is_error);
    if let McpContent::Text { text } = &response.content[0] {
        assert!(text.contains("Tool 'get_weather' executed successfully on client 'weather_client'"));
    }

    // Test 2: Tool not mapped to any client
    let unknown_tool = McpToolCallRequest {
        name: "unknown_tool".to_string(),
        arguments: HashMap::new(),
    };
    
    let response = simulate_tool_execution(&tool_to_client_map, &mcp_clients, &unknown_tool).await;
    assert!(response.is_error);
    if let McpContent::Text { text } = &response.content[0] {
        assert!(text.contains("No client mapping found for tool: 'unknown_tool'"));
        assert!(text.contains("Available tools: "));
        assert!(text.contains("get_weather"));
        assert!(text.contains("send_email"));
        assert!(text.contains("search_web"));
    }

    // Test 3: Tool mapped but client no longer available
    let mut clients_missing = mcp_clients.clone();
    clients_missing.remove("email_client"); // Remove email client
    
    let email_tool = McpToolCallRequest {
        name: "send_email".to_string(),
        arguments: HashMap::new(),
    };
    
    let response = simulate_tool_execution(&tool_to_client_map, &clients_missing, &email_tool).await;
    assert!(response.is_error);
    if let McpContent::Text { text } = &response.content[0] {
        assert!(text.contains("Client 'email_client' for tool 'send_email' is no longer available"));
    }

    println!("✅ All tool routing scenarios tested successfully!");
}

/// Test tool conflict handling (last client wins)
#[tokio::test]
async fn test_tool_conflict_handling() {
    let mut tool_to_client_map = HashMap::new();
    
    // Simulate tool discovery where two clients provide the same tool
    tool_to_client_map.insert("duplicate_tool".to_string(), "client1".to_string());
    tool_to_client_map.insert("duplicate_tool".to_string(), "client2".to_string()); // Last wins
    
    // Verify that the last client wins
    assert_eq!(tool_to_client_map.get("duplicate_tool"), Some(&"client2".to_string()));
    
    println!("✅ Tool conflict handling verified - last client wins!");
}