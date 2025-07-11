use crate::{config::LlmConfig, Error, Result};
use super::types::*;
use async_openai::{config::OpenAIConfig, types as openai_types, Client};
use async_trait::async_trait;
use tracing::{debug, info};

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn create_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse>;
}

pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiClient {
    pub fn new(config: LlmConfig) -> Self {
        let mut openai_config = OpenAIConfig::new()
            .with_api_key(config.api_key);

        if !config.base_url.is_empty() {
            openai_config = openai_config.with_api_base(config.base_url);
        }

        let client = Client::with_config(openai_config);

        Self {
            client,
            model: config.model,
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn create_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        debug!("Creating chat completion with {} messages", request.messages.len());

        // Convert our types to OpenAI types
        let mut messages = Vec::new();
        for msg in request.messages {
            messages.push(msg.to_openai_message()?);
        }

        let tools: Option<Vec<openai_types::ChatCompletionTool>> = if request.tools.is_empty() {
            None
        } else {
            Some(
                request
                    .tools
                    .into_iter()
                    .map(|tool| tool.to_openai_tool())
                    .collect(),
            )
        };

        let mut request_builder = openai_types::CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .temperature(request.temperature.unwrap_or(0.7));

        if let Some(tools) = tools {
            request_builder = request_builder.tools(tools);
        }

        if let Some(max_tokens) = request.max_tokens {
            request_builder = request_builder.max_tokens(max_tokens as u32);
        }

        let openai_request = request_builder.build()?;

        let response = self.client.chat().create(openai_request).await?;

        debug!("Received chat completion response with {} choices", response.choices.len());

        // Convert OpenAI response to our types
        let choices: Vec<Choice> = response
            .choices
            .into_iter()
            .map(|choice| {
                let tool_calls = choice.message.tool_calls.map(|tcs| {
                    tcs.into_iter()
                        .map(|tc| ToolCall {
                            id: tc.id,
                            function: FunctionCall {
                                name: tc.function.name,
                                arguments: tc.function.arguments,
                            },
                        })
                        .collect()
                });

                let message = ChatMessage {
                    role: choice.message.role.to_string(),
                    content: choice.message.content.unwrap_or_default(),
                    tool_calls,
                    tool_call_id: None,
                    name: None,
                };

                Choice {
                    index: choice.index,
                    message,
                    finish_reason: choice.finish_reason.map(|fr| format!("{:?}", fr)),
                }
            })
            .collect();

        let usage = response.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ChatCompletionResponse {
            id: response.id,
            model: response.model,
            choices,
            usage,
        })
    }
}