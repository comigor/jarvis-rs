use jarvis_rust::{
    agent::Agent,
    llm::{Function, Tool},
    mcp::{McpContent, McpToolCallRequest},
};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tokio;

mod common;
use common::{MockLlmClient, MockMcpClient, create_mock_mcp_tool};

/// Test that tools are properly mapped to their respective MCP clients during agent initialization
#[tokio::test]
async fn test_tool_to_client_mapping() {
    // Create mock LLM client
    let mock_llm = MockLlmClient::new();

    // Create mock MCP clients with specific tools
    let client1_tools = vec![
        create_mock_mcp_tool("weather_tool", "Get weather information"),
        create_mock_mcp_tool("news_tool", "Get news articles"),
    ];
    let client2_tools = vec![
        create_mock_mcp_tool("math_tool", "Perform mathematical calculations"),
        create_mock_mcp_tool("translate_tool", "Translate text"),
    ];

    let mock_client1 = MockMcpClient::new().with_tools(client1_tools);
    let mock_client2 = MockMcpClient::new().with_tools(client2_tools);

    // Set up the tool-to-client mapping manually for testing
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert("weather_tool".to_string(), "client1".to_string());
    tool_to_client_map.insert("news_tool".to_string(), "client1".to_string());
    tool_to_client_map.insert("math_tool".to_string(), "client2".to_string());
    tool_to_client_map.insert("translate_tool".to_string(), "client2".to_string());

    // Create available tools for the agent
    let available_tools = vec![
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "weather_tool".to_string(),
                description: "Get weather information".to_string(),
                parameters: serde_json::json!({}),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "news_tool".to_string(),
                description: "Get news articles".to_string(),
                parameters: serde_json::json!({}),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "math_tool".to_string(),
                description: "Perform mathematical calculations".to_string(),
                parameters: serde_json::json!({}),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "translate_tool".to_string(),
                description: "Translate text".to_string(),
                parameters: serde_json::json!({}),
            },
        },
    ];

    // Create MCP clients map
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    mcp_clients.insert("client1".to_string(), Box::new(mock_client1));
    mcp_clients.insert("client2".to_string(), Box::new(mock_client2));

    // Create agent with test constructor
    let agent = Agent::new_for_testing(
        Box::new(mock_llm),
        mcp_clients,
        tool_to_client_map,
        available_tools,
    );

    // Verify tool-to-client mappings
    let tool_map = agent.get_tool_to_client_map();
    assert_eq!(tool_map.get("weather_tool"), Some(&"client1".to_string()));
    assert_eq!(tool_map.get("news_tool"), Some(&"client1".to_string()));
    assert_eq!(tool_map.get("math_tool"), Some(&"client2".to_string()));
    assert_eq!(tool_map.get("translate_tool"), Some(&"client2".to_string()));

    // Verify available tools
    let tools = agent.get_available_tools();
    assert_eq!(tools.len(), 4);

    let tool_names: Vec<String> = tools.iter().map(|t| t.function.name.clone()).collect();
    assert!(tool_names.contains(&"weather_tool".to_string()));
    assert!(tool_names.contains(&"news_tool".to_string()));
    assert!(tool_names.contains(&"math_tool".to_string()));
    assert!(tool_names.contains(&"translate_tool".to_string()));

    // Verify MCP clients exist
    let clients = agent.get_mcp_clients();
    assert!(clients.contains_key("client1"));
    assert!(clients.contains_key("client2"));
    assert_eq!(clients.len(), 2);
}

/// Test that tool name conflicts are handled properly
#[tokio::test]
async fn test_tool_name_conflict_handling() {
    // Create mock LLM client
    let mock_llm = MockLlmClient::new();

    // Create mock MCP clients with conflicting tool names
    let client1_tools = vec![
        create_mock_mcp_tool("duplicate_tool", "Tool from client1"),
        create_mock_mcp_tool("unique_tool1", "Unique tool from client1"),
    ];
    let client2_tools = vec![
        create_mock_mcp_tool("duplicate_tool", "Tool from client2"), // Same name as client1
        create_mock_mcp_tool("unique_tool2", "Unique tool from client2"),
    ];

    let mock_client1 = MockMcpClient::new().with_tools(client1_tools);
    let mock_client2 = MockMcpClient::new().with_tools(client2_tools);

    // Simulate the conflict resolution (last-wins strategy)
    let mut tool_to_client_map = HashMap::new();

    // First client's tools
    tool_to_client_map.insert("duplicate_tool".to_string(), "client1".to_string());
    tool_to_client_map.insert("unique_tool1".to_string(), "client1".to_string());

    // Second client overwrites the duplicate tool (last-wins)
    tool_to_client_map.insert("duplicate_tool".to_string(), "client2".to_string());
    tool_to_client_map.insert("unique_tool2".to_string(), "client2".to_string());

    // Create available tools for the agent
    let available_tools = vec![
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "duplicate_tool".to_string(),
                description: "Tool from client2".to_string(), // Should be from client2 (last wins)
                parameters: serde_json::json!({}),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "unique_tool1".to_string(),
                description: "Unique tool from client1".to_string(),
                parameters: serde_json::json!({}),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "unique_tool2".to_string(),
                description: "Unique tool from client2".to_string(),
                parameters: serde_json::json!({}),
            },
        },
    ];

    // Create MCP clients map
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    mcp_clients.insert("client1".to_string(), Box::new(mock_client1));
    mcp_clients.insert("client2".to_string(), Box::new(mock_client2));

    // Create agent with test constructor
    let agent = Agent::new_for_testing(
        Box::new(mock_llm),
        mcp_clients,
        tool_to_client_map,
        available_tools,
    );

    // Verify that the duplicate tool maps to client2 (last-wins strategy)
    let tool_map = agent.get_tool_to_client_map();
    assert_eq!(tool_map.get("duplicate_tool"), Some(&"client2".to_string()));
    assert_eq!(tool_map.get("unique_tool1"), Some(&"client1".to_string()));
    assert_eq!(tool_map.get("unique_tool2"), Some(&"client2".to_string()));

    // Verify that we have 3 tools total (duplicate was overwritten)
    assert_eq!(tool_map.len(), 3);

    // Verify the available tools list contains the expected tools
    let tools = agent.get_available_tools();
    assert_eq!(tools.len(), 3);

    let tool_names: Vec<String> = tools.iter().map(|t| t.function.name.clone()).collect();
    assert!(tool_names.contains(&"duplicate_tool".to_string()));
    assert!(tool_names.contains(&"unique_tool1".to_string()));
    assert!(tool_names.contains(&"unique_tool2".to_string()));
}

/// Test that tool execution fails gracefully when client is not found
#[tokio::test]
async fn test_tool_execution_missing_client() {
    // Create mock LLM client
    let mock_llm = MockLlmClient::new();

    // Create mock MCP client
    let client_tools = vec![create_mock_mcp_tool("test_tool", "Test tool")];
    let mock_client = MockMcpClient::new().with_tools(client_tools);

    // Set up tool-to-client mapping
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert("test_tool".to_string(), "missing_client".to_string()); // Map to non-existent client

    // Create available tools
    let available_tools = vec![Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "test_tool".to_string(),
            description: "Test tool".to_string(),
            parameters: serde_json::json!({}),
        },
    }];

    // Create MCP clients map with different client name
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    mcp_clients.insert("existing_client".to_string(), Box::new(mock_client));

    // Create agent
    let mut agent = Agent::new_for_testing(
        Box::new(mock_llm),
        mcp_clients,
        tool_to_client_map,
        available_tools,
    );

    // Create a tool call request
    let tool_call = McpToolCallRequest {
        name: "test_tool".to_string(),
        arguments: HashMap::new(),
    };

    // Execute the tool and expect an error
    let response = agent.execute_mcp_tool_for_testing(&tool_call).await;

    // Verify that the response indicates an error
    assert!(response.is_error);
    assert_eq!(response.content.len(), 1);

    if let McpContent::Text { text } = &response.content[0] {
        assert!(
            text.contains("Client 'missing_client' for tool 'test_tool' is no longer available")
        );
    } else {
        panic!("Expected text content in error response");
    }
}

/// Test that tool execution fails gracefully when tool is not mapped
#[tokio::test]
async fn test_tool_execution_unmapped_tool() {
    // Create mock LLM client
    let mock_llm = MockLlmClient::new();

    // Create mock MCP client with some tools
    let client_tools = vec![create_mock_mcp_tool("existing_tool", "An existing tool")];
    let mock_client = MockMcpClient::new().with_tools(client_tools);

    // Set up tool-to-client mapping with only one tool
    let mut tool_to_client_map = HashMap::new();
    tool_to_client_map.insert("existing_tool".to_string(), "client1".to_string());

    // Create available tools
    let available_tools = vec![Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "existing_tool".to_string(),
            description: "An existing tool".to_string(),
            parameters: serde_json::json!({}),
        },
    }];

    // Create MCP clients map
    let mut mcp_clients: HashMap<String, Box<dyn jarvis_rust::mcp::McpClient>> = HashMap::new();
    mcp_clients.insert("client1".to_string(), Box::new(mock_client));

    // Create agent
    let mut agent = Agent::new_for_testing(
        Box::new(mock_llm),
        mcp_clients,
        tool_to_client_map,
        available_tools,
    );

    // Create a tool call request for a non-existent tool
    let tool_call = McpToolCallRequest {
        name: "nonexistent_tool".to_string(),
        arguments: HashMap::new(),
    };

    // Execute the tool and expect an error
    let response = agent.execute_mcp_tool_for_testing(&tool_call).await;

    // Verify that the response indicates an error
    assert!(response.is_error);
    assert_eq!(response.content.len(), 1);

    if let McpContent::Text { text } = &response.content[0] {
        assert!(text.contains("No client mapping found for tool: 'nonexistent_tool'"));
        assert!(text.contains("Available tools: existing_tool"));
    } else {
        panic!("Expected text content in error response");
    }
}
