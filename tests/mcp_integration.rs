use jarvis_rust::{
    config::{McpServerConfig, McpClientType},
    mcp::{
        McpClient, McpInitializeRequest, McpClientCapabilities, McpRootsCapability,
        McpToolCallRequest, McpToolCallResponse, McpContent, McpTool,
        McpGetPromptRequest, McpPrompt, McpPromptArgument,
    },
    Error,
};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio;

mod common;
use common::{MockMcpClient, create_mock_mcp_tool, create_mock_tool_response, create_mock_tool_error_response};

#[tokio::test]
async fn test_mcp_client_initialization() {
    let mut mock_client = MockMcpClient::new();
    
    let init_request = McpInitializeRequest {
        capabilities: McpClientCapabilities {
            roots: Some(McpRootsCapability {
                list_changed: false,
            }),
            sampling: None,
        },
    };

    let result = mock_client.initialize(init_request).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.protocol_version, "2024-11-05");
    assert_eq!(response.server_info.as_ref().unwrap().name, "mock-server");
    assert_eq!(response.server_info.as_ref().unwrap().version, "1.0.0");
}

#[tokio::test]
async fn test_mcp_client_initialization_failure() {
    let mut mock_client = MockMcpClient::new()
        .with_initialize_error("Initialization failed".to_string());

    let init_request = McpInitializeRequest {
        capabilities: McpClientCapabilities {
            roots: Some(McpRootsCapability {
                list_changed: false,
            }),
            sampling: None,
        },
    };

    let result = mock_client.initialize(init_request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Initialization failed"));
}

#[tokio::test]
async fn test_mcp_tool_discovery() {
    let tools = vec![
        create_mock_mcp_tool("get_weather", "Get current weather for a location"),
        create_mock_mcp_tool("send_email", "Send an email to a recipient"),
        create_mock_mcp_tool("create_file", "Create a new file with content"),
    ];

    let mock_client = MockMcpClient::new().with_tools(tools.clone());

    let discovered_tools = mock_client.list_tools().await.unwrap();
    
    assert_eq!(discovered_tools.len(), 3);
    assert_eq!(discovered_tools[0].name, "get_weather");
    assert_eq!(discovered_tools[1].name, "send_email");
    assert_eq!(discovered_tools[2].name, "create_file");

    // Verify tool descriptions
    assert_eq!(discovered_tools[0].description, "Get current weather for a location");
    assert_eq!(discovered_tools[1].description, "Send an email to a recipient");
    assert_eq!(discovered_tools[2].description, "Create a new file with content");
}

#[tokio::test]
async fn test_mcp_tool_execution_success() {
    let mock_client = MockMcpClient::new()
        .with_tool_response(
            "get_weather".to_string(),
            create_mock_tool_response("Current weather in London: Sunny, 22°C")
        );

    let tool_request = McpToolCallRequest {
        name: "get_weather".to_string(),
        arguments: {
            let mut args = HashMap::new();
            args.insert("location".to_string(), Value::String("London".to_string()));
            args
        },
    };

    let response = mock_client.call_tool(tool_request).await.unwrap();
    
    assert!(!response.is_error);
    assert_eq!(response.content.len(), 1);
    
    if let McpContent::Text { text } = &response.content[0] {
        assert_eq!(text, "Current weather in London: Sunny, 22°C");
    } else {
        panic!("Expected text content");
    }
}

#[tokio::test]
async fn test_mcp_tool_execution_failure() {
    let mock_client = MockMcpClient::new()
        .with_tool_error("broken_tool".to_string(), "Tool execution failed".to_string());

    let tool_request = McpToolCallRequest {
        name: "broken_tool".to_string(),
        arguments: HashMap::new(),
    };

    let result = mock_client.call_tool(tool_request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Tool execution failed"));
}

#[tokio::test]
async fn test_mcp_tool_execution_with_complex_arguments() {
    let mock_client = MockMcpClient::new()
        .with_tool_response(
            "send_email".to_string(),
            create_mock_tool_response("Email sent successfully to john@example.com")
        );

    let tool_request = McpToolCallRequest {
        name: "send_email".to_string(),
        arguments: {
            let mut args = HashMap::new();
            args.insert("to".to_string(), Value::String("john@example.com".to_string()));
            args.insert("subject".to_string(), Value::String("Test Subject".to_string()));
            args.insert("body".to_string(), Value::String("Hello, this is a test email.".to_string()));
            args.insert("priority".to_string(), Value::String("high".to_string()));
            args
        },
    };

    let response = mock_client.call_tool(tool_request).await.unwrap();
    
    assert!(!response.is_error);
    if let McpContent::Text { text } = &response.content[0] {
        assert!(text.contains("john@example.com"));
        assert!(text.contains("sent successfully"));
    }
}

#[tokio::test]
async fn test_mcp_tool_not_found() {
    let mock_client = MockMcpClient::new();

    let tool_request = McpToolCallRequest {
        name: "nonexistent_tool".to_string(),
        arguments: HashMap::new(),
    };

    // The default mock client should handle unknown tools gracefully
    let response = mock_client.call_tool(tool_request).await.unwrap();
    
    assert!(!response.is_error);
    if let McpContent::Text { text } = &response.content[0] {
        assert!(text.contains("Mock response for tool: nonexistent_tool"));
    }
}

#[tokio::test]
async fn test_mcp_prompt_discovery() {
    let prompts = vec![
        McpPrompt {
            name: "system_prompt".to_string(),
            description: "System prompt for the agent".to_string(),
            arguments: vec![], // No arguments means it's a system prompt
        },
        McpPrompt {
            name: "user_prompt".to_string(),
            description: "User-specific prompt template".to_string(),
            arguments: vec![McpPromptArgument {
                name: "user_name".to_string(),
                description: "Name of the user".to_string(),
                required: true,
            }],
        },
    ];

    let mock_client = MockMcpClient::new();
    *mock_client.prompts.lock().unwrap() = prompts.clone();

    let discovered_prompts = mock_client.list_prompts().await.unwrap();
    
    assert_eq!(discovered_prompts.len(), 2);
    assert_eq!(discovered_prompts[0].name, "system_prompt");
    assert_eq!(discovered_prompts[1].name, "user_prompt");
    
    // Check system prompt (no arguments)
    assert!(discovered_prompts[0].arguments.is_empty());
    
    // Check user prompt (has arguments)
    assert_eq!(discovered_prompts[1].arguments.len(), 1);
    assert_eq!(discovered_prompts[1].arguments[0].name, "user_name");
    assert_eq!(discovered_prompts[1].arguments[0].required, true);
}

#[tokio::test]
async fn test_mcp_prompt_retrieval() {
    let mock_client = MockMcpClient::new();

    let prompt_request = McpGetPromptRequest {
        name: "system_prompt".to_string(),
        arguments: HashMap::new(),
    };

    let response = mock_client.get_prompt(prompt_request).await.unwrap();
    
    assert_eq!(response.description, "Mock prompt".to_string());
    assert_eq!(response.messages.len(), 1);
    assert_eq!(response.messages[0].role, "assistant");
    
    if let McpContent::Text { text } = &response.messages[0].content {
        assert_eq!(text, "Mock prompt content");
    }
}

#[tokio::test]
async fn test_mcp_client_capabilities() {
    let capabilities = McpClientCapabilities {
        roots: Some(McpRootsCapability {
            list_changed: true,
        }),
        sampling: None,
    };

    // Test that capabilities can be serialized and contain expected fields
    let serialized = serde_json::to_string(&capabilities).unwrap();
    assert!(serialized.contains("roots"));
    assert!(serialized.contains("list_changed"));
}

#[tokio::test]
async fn test_mcp_content_types() {
    // Test text content
    let text_content = McpContent::Text {
        text: "This is a text response".to_string(),
    };

    match &text_content {
        McpContent::Text { text } => {
            assert_eq!(text, "This is a text response");
        }
        _ => panic!("Expected text content"),
    }

    // Test content serialization
    let serialized = serde_json::to_string(&text_content).unwrap();
    let deserialized: McpContent = serde_json::from_str(&serialized).unwrap();
    
    if let McpContent::Text { text } = deserialized {
        assert_eq!(text, "This is a text response");
    }
}

#[tokio::test]
async fn test_mcp_error_response() {
    let error_response = create_mock_tool_error_response("Network timeout occurred");
    
    assert!(error_response.is_error);
    assert_eq!(error_response.content.len(), 1);
    
    if let McpContent::Text { text } = &error_response.content[0] {
        assert_eq!(text, "Network timeout occurred");
    }
}

#[tokio::test]
async fn test_mcp_tool_schema_validation() {
    let tool = create_mock_mcp_tool("calculate", "Perform mathematical calculations");
    
    // Verify the tool has the expected structure
    assert_eq!(tool.name, "calculate");
    assert_eq!(tool.description, "Perform mathematical calculations");
    assert!(tool.input_schema.is_object());
    
    // Verify the schema contains expected properties
    let schema = &tool.input_schema;
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());
    assert!(schema["properties"]["input"].is_object());
}

#[tokio::test]
async fn test_concurrent_mcp_tool_calls() {
    let mock_client = MockMcpClient::new()
        .with_tool_response(
            "fast_tool".to_string(),
            create_mock_tool_response("Fast response")
        )
        .with_tool_response(
            "slow_tool".to_string(),
            create_mock_tool_response("Slow response")
        );

    let client = std::sync::Arc::new(tokio::sync::Mutex::new(mock_client));
    let mut handles = vec![];

    // Make concurrent tool calls
    for i in 0..5 {
        let client_clone = client.clone();
        let tool_name = if i % 2 == 0 { "fast_tool" } else { "slow_tool" };
        let handle = tokio::spawn(async move {
            let client = client_clone.lock().await;
            let request = McpToolCallRequest {
                name: tool_name.to_string(),
                arguments: HashMap::new(),
            };
            client.call_tool(request).await
        });
        handles.push(handle);
    }

    // Wait for all calls to complete
    let mut results = vec![];
    for handle in handles {
        let result = handle.await.unwrap();
        results.push(result);
    }

    // Verify all calls succeeded
    assert_eq!(results.len(), 5);
    for result in results {
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.is_error);
    }
}

#[tokio::test]
async fn test_mcp_server_config_types() {
    // Test SSE configuration
    let sse_config = McpServerConfig {
        name: "sse-server".to_string(),
        client_type: McpClientType::Sse,
        url: Some("http://localhost:3000/sse".to_string()),
        command: None,
        args: vec![],
        env: HashMap::new(),
        headers: HashMap::new(),
    };

    assert_eq!(sse_config.name, "sse-server");
    assert!(matches!(sse_config.client_type, McpClientType::Sse));
    assert!(sse_config.url.is_some());
    assert!(sse_config.command.is_none());

    // Test Stdio configuration
    let stdio_config = McpServerConfig {
        name: "stdio-server".to_string(),
        client_type: McpClientType::Stdio,
        url: None,
        command: Some("./server".to_string()),
        args: vec!["--verbose".to_string()],
        env: {
            let mut env = HashMap::new();
            env.insert("DEBUG".to_string(), "true".to_string());
            env
        },
        headers: HashMap::new(),
    };

    assert_eq!(stdio_config.name, "stdio-server");
    assert!(matches!(stdio_config.client_type, McpClientType::Stdio));
    assert!(stdio_config.url.is_none());
    assert_eq!(stdio_config.command, Some("./server".to_string()));
    assert_eq!(stdio_config.args.len(), 1);
    assert_eq!(stdio_config.env.get("DEBUG"), Some(&"true".to_string()));

    // Test HTTP configuration
    let http_config = McpServerConfig {
        name: "http-server".to_string(),
        client_type: McpClientType::StreamableHttp,
        url: Some("http://api.example.com".to_string()),
        command: None,
        args: vec![],
        env: HashMap::new(),
        headers: {
            let mut headers = HashMap::new();
            headers.insert("Authorization".to_string(), "Bearer token123".to_string());
            headers
        },
    };

    assert_eq!(http_config.name, "http-server");
    assert!(matches!(http_config.client_type, McpClientType::StreamableHttp));
    assert!(http_config.url.is_some());
    assert_eq!(http_config.headers.get("Authorization"), Some(&"Bearer token123".to_string()));
}

#[tokio::test]
async fn test_mcp_client_type_serialization() {
    // Test that MCP client types serialize correctly
    let sse_type = McpClientType::Sse;
    let http_type = McpClientType::StreamableHttp;
    let stdio_type = McpClientType::Stdio;

    let sse_json = serde_json::to_string(&sse_type).unwrap();
    let http_json = serde_json::to_string(&http_type).unwrap();
    let stdio_json = serde_json::to_string(&stdio_type).unwrap();

    assert_eq!(sse_json, "\"sse\"");
    assert_eq!(http_json, "\"streamable_http\"");
    assert_eq!(stdio_json, "\"stdio\"");

    // Test deserialization
    let sse_deserialized: McpClientType = serde_json::from_str(&sse_json).unwrap();
    let http_deserialized: McpClientType = serde_json::from_str(&http_json).unwrap();
    let stdio_deserialized: McpClientType = serde_json::from_str(&stdio_json).unwrap();

    assert!(matches!(sse_deserialized, McpClientType::Sse));
    assert!(matches!(http_deserialized, McpClientType::StreamableHttp));
    assert!(matches!(stdio_deserialized, McpClientType::Stdio));
}