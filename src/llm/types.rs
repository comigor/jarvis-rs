use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessageArgs,
    ChatCompletionRequestUserMessageContent, ChatCompletionTool, FunctionObject,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<Tool>,
    pub max_tokens: Option<u16>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ChatMessage {
    pub fn to_openai_message(&self) -> Result<ChatCompletionRequestMessage, crate::Error> {
        match self.role.as_str() {
            "system" => {
                let msg = ChatCompletionRequestSystemMessageArgs::default()
                    .content(ChatCompletionRequestSystemMessageContent::Text(
                        self.content.clone(),
                    ))
                    .build()
                    .map_err(|e| {
                        crate::Error::llm(format!("Failed to build system message: {}", e))
                    })?;
                Ok(msg.into())
            }
            "user" => {
                let mut builder = ChatCompletionRequestUserMessageArgs::default();
                builder.content(ChatCompletionRequestUserMessageContent::Text(
                    self.content.clone(),
                ));
                if let Some(ref name) = self.name {
                    builder.name(name);
                }
                let msg = builder.build().map_err(|e| {
                    crate::Error::llm(format!("Failed to build user message: {}", e))
                })?;
                Ok(msg.into())
            }
            "assistant" => {
                let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
                if !self.content.is_empty() {
                    builder.content(ChatCompletionRequestAssistantMessageContent::Text(
                        self.content.clone(),
                    ));
                }
                if let Some(ref tool_calls) = self.tool_calls {
                    let openai_tool_calls: Vec<async_openai::types::ChatCompletionMessageToolCall> =
                        tool_calls
                            .iter()
                            .map(|tc| async_openai::types::ChatCompletionMessageToolCall {
                                id: tc.id.clone(),
                                r#type: async_openai::types::ChatCompletionToolType::Function,
                                function: async_openai::types::FunctionCall {
                                    name: tc.function.name.clone(),
                                    arguments: tc.function.arguments.clone(),
                                },
                            })
                            .collect();
                    builder.tool_calls(openai_tool_calls);
                }
                let msg = builder.build().map_err(|e| {
                    crate::Error::llm(format!("Failed to build assistant message: {}", e))
                })?;
                Ok(msg.into())
            }
            "tool" => {
                let msg = ChatCompletionRequestToolMessageArgs::default()
                    .content(ChatCompletionRequestToolMessageContent::Text(
                        self.content.clone(),
                    ))
                    .tool_call_id(self.tool_call_id.as_ref().unwrap_or(&String::new()))
                    .build()
                    .map_err(|e| {
                        crate::Error::llm(format!("Failed to build tool message: {}", e))
                    })?;
                Ok(msg.into())
            }
            _ => Err(crate::Error::llm(format!(
                "Unknown message role: {}",
                self.role
            ))),
        }
    }
}

impl Tool {
    pub fn to_openai_tool(&self) -> ChatCompletionTool {
        ChatCompletionTool {
            r#type: async_openai::types::ChatCompletionToolType::Function,
            function: FunctionObject {
                name: self.function.name.clone(),
                description: Some(self.function.description.clone()),
                parameters: Some(self.function.parameters.clone()),
                strict: None,
            },
        }
    }
}
