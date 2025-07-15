use async_openai::types::ChatCompletionRequestMessage;
use jarvis_rust::{
    config::LlmConfig,
    llm::{
        ChatCompletionChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Function,
        FunctionCall, OpenAiClient, Tool, ToolCall, Usage,
    },
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn create_test_config() -> LlmConfig {
    LlmConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: "test-api-key".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: Some("Test prompt".to_string()),
    }
}

#[test]
fn test_openai_client_creation() {
    let config = create_test_config();
    let _client = OpenAiClient::new(config.clone());
    // Note: model field is private, we'll test functionality instead of internal structure
}

#[test]
fn test_openai_client_with_custom_base_url() {
    let mut config = create_test_config();
    config.base_url = "https://custom.api.com".to_string();

    let _client = OpenAiClient::new(config);
    // Note: model field is private, we'll test functionality instead of internal structure
}

#[test]
fn test_chat_message_to_openai_system() {
    let msg = ChatMessage {
        role: "system".to_string(),
        content: "You are a helpful assistant".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let openai_msg = msg.to_openai_message().unwrap();
    // Verify it's a system message by attempting to extract content
    // Note: Actual verification depends on openai crate's internal structure
    assert!(matches!(
        openai_msg,
        ChatCompletionRequestMessage::System(_)
    ));
}

#[test]
fn test_chat_message_to_openai_user() {
    let msg = ChatMessage {
        role: "user".to_string(),
        content: "Hello, how are you?".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: Some("test_user".to_string()),
    };

    let openai_msg = msg.to_openai_message().unwrap();
    assert!(matches!(openai_msg, ChatCompletionRequestMessage::User(_)));
}

#[test]
fn test_chat_message_to_openai_assistant() {
    let msg = ChatMessage {
        role: "assistant".to_string(),
        content: "I'm doing well, thank you!".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let openai_msg = msg.to_openai_message().unwrap();
    assert!(matches!(
        openai_msg,
        ChatCompletionRequestMessage::Assistant(_)
    ));
}

#[test]
fn test_chat_message_to_openai_assistant_with_tool_calls() {
    let tool_calls = vec![ToolCall {
        id: "call_123".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "get_weather".to_string(),
            arguments: r#"{"location": "London"}"#.to_string(),
        },
    }];

    let msg = ChatMessage {
        role: "assistant".to_string(),
        content: "".to_string(),
        tool_calls: Some(tool_calls),
        tool_call_id: None,
        name: None,
    };

    let openai_msg = msg.to_openai_message().unwrap();
    assert!(matches!(
        openai_msg,
        ChatCompletionRequestMessage::Assistant(_)
    ));
}

#[test]
fn test_chat_message_to_openai_tool() {
    let msg = ChatMessage {
        role: "tool".to_string(),
        content: "The weather in London is sunny".to_string(),
        tool_calls: None,
        tool_call_id: Some("call_123".to_string()),
        name: None,
    };

    let openai_msg = msg.to_openai_message().unwrap();
    assert!(matches!(openai_msg, ChatCompletionRequestMessage::Tool(_)));
}

#[test]
fn test_chat_message_invalid_role() {
    let msg = ChatMessage {
        role: "invalid_role".to_string(),
        content: "This should fail".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let result = msg.to_openai_message();
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Unknown message role")
    );
}

#[test]
fn test_tool_to_openai_tool() {
    let tool = Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "get_weather".to_string(),
            description: "Get current weather for a location".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city name"
                    }
                },
                "required": ["location"]
            }),
        },
    };

    let openai_tool = tool.to_openai_tool();
    assert_eq!(openai_tool.function.name, "get_weather");
    assert_eq!(
        openai_tool.function.description,
        Some("Get current weather for a location".to_string())
    );
    assert!(openai_tool.function.parameters.is_some());
}

#[test]
fn test_chat_completion_request_creation() {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are helpful".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let tools = vec![Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: json!({"type": "object"}),
        },
    }];

    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages,
        tools,
        max_tokens: Some(150),
        temperature: Some(0.7),
    };

    assert_eq!(request.model, "gpt-4");
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.max_tokens, Some(150));
    assert_eq!(request.temperature, Some(0.7));
}

#[test]
fn test_tool_call_and_function_call_serialization() {
    let function_call = FunctionCall {
        name: "get_weather".to_string(),
        arguments: r#"{"location": "New York"}"#.to_string(),
    };

    let tool_call = ToolCall {
        id: "call_abc123".to_string(),
        call_type: "function".to_string(),
        function: function_call,
    };

    // Test serialization
    let serialized = serde_json::to_string(&tool_call).unwrap();
    assert!(serialized.contains("call_abc123"));
    assert!(serialized.contains("get_weather"));
    assert!(serialized.contains("New York"));

    // Test deserialization
    let deserialized: ToolCall = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.id, "call_abc123");
    assert_eq!(deserialized.function.name, "get_weather");
    assert!(deserialized.function.arguments.contains("New York"));
}

#[test]
fn test_usage_serialization() {
    let usage = Usage {
        prompt_tokens: 50,
        completion_tokens: 25,
        total_tokens: 75,
    };

    let serialized = serde_json::to_string(&usage).unwrap();
    let deserialized: Usage = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.prompt_tokens, 50);
    assert_eq!(deserialized.completion_tokens, 25);
    assert_eq!(deserialized.total_tokens, 75);
}

#[test]
fn test_function_serialization() {
    let function = Function {
        name: "calculate".to_string(),
        description: "Perform calculations".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string"
                }
            }
        }),
    };

    let serialized = serde_json::to_string(&function).unwrap();
    let deserialized: Function = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.name, "calculate");
    assert_eq!(deserialized.description, "Perform calculations");
    assert!(deserialized.parameters.is_object());
}

#[test]
fn test_tool_serialization() {
    let tool = Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "test_func".to_string(),
            description: "Test function".to_string(),
            parameters: json!({"type": "object"}),
        },
    };

    let serialized = serde_json::to_string(&tool).unwrap();
    assert!(serialized.contains("\"type\":\"function\""));
    assert!(serialized.contains("test_func"));

    let deserialized: Tool = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.tool_type, "function");
    assert_eq!(deserialized.function.name, "test_func");
}

#[test]
fn test_chat_message_cloning() {
    let original = ChatMessage {
        role: "user".to_string(),
        content: "Test message".to_string(),
        tool_calls: Some(vec![ToolCall {
            id: "test_id".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "test_func".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
        tool_call_id: Some("test_call_id".to_string()),
        name: Some("test_name".to_string()),
    };

    let cloned = original.clone();
    assert_eq!(original.role, cloned.role);
    assert_eq!(original.content, cloned.content);
    assert_eq!(original.tool_call_id, cloned.tool_call_id);
    assert_eq!(original.name, cloned.name);
    assert!(original.tool_calls.is_some() && cloned.tool_calls.is_some());
}

#[test]
fn test_choice_creation() {
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "Test response".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let choice = ChatCompletionChoice {
        index: 0,
        message,
        finish_reason: Some("stop".to_string()),
    };

    assert_eq!(choice.index, 0);
    assert_eq!(choice.message.content, "Test response");
    assert_eq!(choice.finish_reason, Some("stop".to_string()));
}

#[test]
fn test_chat_completion_response_creation() {
    let response = ChatCompletionResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1234567890,
        model: "gpt-4".to_string(),
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: "Hello!".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
    };

    assert_eq!(response.id, "chatcmpl-123");
    assert_eq!(response.model, "gpt-4");
    assert_eq!(response.choices.len(), 1);
    assert!(response.usage.is_some());
    assert_eq!(response.usage.unwrap().total_tokens, 15);
}
