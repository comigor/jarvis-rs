use crate::{
    config::McpServerConfig,
    mcp::{McpContent, McpToolCallRequest, McpToolCallResponse},
    Error, Result,
};
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use rmcp::{
    model::{CallToolRequestParam, ClientCapabilities, ClientInfo, Implementation},
    service::{RunningService, ServiceExt},
    transport::{
        sse_client::SseClientConfig, streamable_http_client::StreamableHttpClientTransportConfig,
        ConfigureCommandExt, SseClientTransport, StreamableHttpClientTransport, TokioChildProcess,
    },
    RoleClient,
};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Wrapper around rmcp client to provide compatibility with our existing MCP interface
pub struct RmcpClient {
    name: String,
    config: McpServerConfig,
    peer: Option<RunningService<RoleClient, rmcp::model::InitializeRequestParam>>,
}

impl RmcpClient {
    pub async fn new(config: McpServerConfig) -> Result<Self> {
        info!("Creating new rmcp client for: {}", config.name);

        let mut client = Self {
            name: config.name.clone(),
            config,
            peer: None,
        };

        // Initialize the rmcp service
        client.initialize_service().await?;

        Ok(client)
    }

    async fn initialize_service(&mut self) -> Result<()> {
        debug!("Initializing rmcp service for: {}", self.name);

        match self.config.client_type {
            crate::config::McpClientType::Stdio => self.initialize_stdio_service().await,
            crate::config::McpClientType::Sse => self.initialize_sse_service().await,
            crate::config::McpClientType::StreamableHttp => self.initialize_http_service().await,
        }
    }

    async fn initialize_stdio_service(&mut self) -> Result<()> {
        let command = self.config.command.as_ref().ok_or_else(|| {
            Error::config("Stdio MCP client requires 'command' field".to_string())
        })?;

        debug!("Creating stdio process for command: {}", command);

        let mut cmd = Command::new(command);

        // Add arguments if provided
        for arg in &self.config.args {
            cmd.arg(arg);
        }

        // Add environment variables if provided
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        // Create the rmcp service using the pattern from the example
        let transport = TokioChildProcess::new(cmd.configure(|_| {}))?;

        // Create client info for MCP protocol compliance
        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "jarvis-rust".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let peer = client_info
            .serve(transport)
            .await
            .map_err(|e| Error::mcp(format!("Failed to create rmcp service: {}", e)))?;

        info!("Successfully created rmcp service for: {}", self.name);

        // Store the peer for later use
        self.peer = Some(peer);

        Ok(())
    }

    async fn initialize_sse_service(&mut self) -> Result<()> {
        let url = self
            .config
            .url
            .as_ref()
            .ok_or_else(|| Error::config("SSE MCP client requires 'url' field".to_string()))?;

        debug!("Creating SSE connection to: {}", url);

        let transport = if self.config.headers.is_empty() {
            // Simple case without headers
            SseClientTransport::start(url.clone())
                .await
                .map_err(|e| Error::mcp(format!("Failed to create SSE transport: {}", e)))?
        } else {
            // Custom client with headers
            let mut headers = HeaderMap::new();
            for (key, value) in &self.config.headers {
                let header_name: reqwest::header::HeaderName = key
                    .parse()
                    .map_err(|e| Error::config(format!("Invalid header name '{}': {}", key, e)))?;
                let header_value: reqwest::header::HeaderValue = value.parse().map_err(|e| {
                    Error::config(format!("Invalid header value for '{}': {}", key, e))
                })?;
                headers.insert(header_name, header_value);
            }

            let client = reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .map_err(|e| Error::mcp(format!("Failed to create HTTP client: {}", e)))?;

            SseClientTransport::start_with_client(
                client,
                SseClientConfig {
                    sse_endpoint: url.clone().into(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| Error::mcp(format!("Failed to create SSE transport: {}", e)))?
        };

        // Create client info for MCP protocol compliance
        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "jarvis-rust".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let peer = client_info
            .serve(transport)
            .await
            .map_err(|e| Error::mcp(format!("Failed to serve SSE rmcp service: {}", e)))?;

        info!("Successfully created SSE rmcp service for: {}", self.name);

        // Store the peer for later use
        self.peer = Some(peer);

        Ok(())
    }

    async fn initialize_http_service(&mut self) -> Result<()> {
        let url = self
            .config
            .url
            .as_ref()
            .ok_or_else(|| Error::config("HTTP MCP client requires 'url' field".to_string()))?;

        debug!("Creating HTTP connection to: {}", url);

        // Create HTTP client with headers (same pattern as SSE transport)
        let mut headers = HeaderMap::new();
        for (key, value) in &self.config.headers {
            let header_name: reqwest::header::HeaderName = key
                .parse()
                .map_err(|e| Error::config(format!("Invalid header name '{}': {}", key, e)))?;
            let header_value: reqwest::header::HeaderValue = value
                .parse()
                .map_err(|e| Error::config(format!("Invalid header value for '{}': {}", key, e)))?;
            headers.insert(header_name, header_value);
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| Error::mcp(format!("Failed to create HTTP client: {}", e)))?;

        // Use with_client instead of from_uri to support custom headers
        let transport = StreamableHttpClientTransport::with_client(
            client,
            StreamableHttpClientTransportConfig::with_uri(url.clone()),
        );

        // Create client info for MCP protocol compliance
        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "jarvis-rust".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let peer = client_info
            .serve(transport)
            .await
            .map_err(|e| Error::mcp(format!("Failed to serve HTTP rmcp service: {}", e)))?;

        info!("Successfully created HTTP rmcp service for: {}", self.name);

        // Store the peer for later use
        self.peer = Some(peer);

        Ok(())
    }
}

#[async_trait]
impl crate::mcp::McpClient for RmcpClient {
    async fn initialize(
        &mut self,
        _request: crate::mcp::McpInitializeRequest,
    ) -> Result<crate::mcp::McpInitializeResponse> {
        // rmcp handles initialization internally, so we just return a success response
        Ok(crate::mcp::McpInitializeResponse {
            capabilities: crate::mcp::McpServerCapabilities {
                tools: Some(crate::mcp::McpToolsCapability {
                    list_changed: false,
                }),
                prompts: Some(crate::mcp::McpPromptsCapability {
                    list_changed: false,
                }),
                resources: None,
            },
            protocol_version: "1.0".to_string(),
            server_info: Some(crate::mcp::McpServerInfo {
                name: self.name.clone(),
                version: "1.0".to_string(),
            }),
        })
    }

    async fn list_tools(&self) -> Result<Vec<crate::mcp::McpTool>> {
        if let Some(ref peer) = self.peer {
            debug!("Listing tools from rmcp peer: {}", self.name);

            match peer.list_tools(Default::default()).await {
                Ok(tools) => {
                    info!(
                        "Listed {} tools from rmcp peer: {}",
                        tools.tools.len(),
                        self.name
                    );

                    // Convert rmcp tools to our format
                    let converted_tools = tools
                        .tools
                        .into_iter()
                        .map(|tool| crate::mcp::McpTool {
                            name: tool.name.to_string(),
                            description: tool
                                .description
                                .map(|d| d.to_string())
                                .unwrap_or_default(),
                            input_schema: serde_json::Value::Object((*tool.input_schema).clone()),
                        })
                        .collect();

                    Ok(converted_tools)
                }
                Err(e) => {
                    warn!("Failed to list tools from rmcp peer {}: {}", self.name, e);
                    Err(Error::mcp(format!("Failed to list tools: {}", e)))
                }
            }
        } else {
            warn!("rmcp peer not initialized for: {}", self.name);
            Ok(Vec::new())
        }
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> Result<McpToolCallResponse> {
        if let Some(ref peer) = self.peer {
            debug!(
                "Calling tool: {} with rmcp peer: {}",
                request.name, self.name
            );

            // Convert arguments to the format expected by rmcp
            let arguments = if request.arguments.is_empty() {
                None
            } else {
                Some(serde_json::Map::from_iter(
                    request
                        .arguments
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone())),
                ))
            };

            let rmcp_request = CallToolRequestParam {
                name: request.name.clone().into(),
                arguments,
            };

            match peer.call_tool(rmcp_request).await {
                Ok(result) => {
                    debug!("Tool {} called successfully via rmcp", request.name);

                    // Convert rmcp result to our format
                    let content = result
                        .content
                        .into_iter()
                        .map(|content_item| {
                            // For now, convert all content to text format
                            // TODO: Handle different content types properly
                            McpContent::Text {
                                text: format!("{:?}", content_item),
                            }
                        })
                        .collect();

                    Ok(McpToolCallResponse {
                        content,
                        is_error: result.is_error.unwrap_or(false),
                    })
                }
                Err(e) => {
                    warn!(
                        "Failed to call tool {} via rmcp peer {}: {}",
                        request.name, self.name, e
                    );
                    Err(Error::mcp(format!("Tool call failed: {}", e)))
                }
            }
        } else {
            warn!("rmcp peer not initialized for: {}", self.name);
            Err(Error::mcp("rmcp peer not initialized".to_string()))
        }
    }

    async fn list_prompts(&self) -> Result<Vec<crate::mcp::McpPrompt>> {
        if let Some(ref peer) = self.peer {
            debug!("Listing prompts from rmcp peer: {}", self.name);

            match peer.list_prompts(Default::default()).await {
                Ok(prompts_result) => {
                    info!(
                        "Listed {} prompts from rmcp peer: {}",
                        prompts_result.prompts.len(),
                        self.name
                    );

                    // Convert rmcp prompts to our format
                    let converted_prompts = prompts_result
                        .prompts
                        .into_iter()
                        .map(|prompt| crate::mcp::McpPrompt {
                            name: prompt.name,
                            description: prompt.description.unwrap_or_default(),
                            arguments: prompt
                                .arguments
                                .unwrap_or_default()
                                .into_iter()
                                .map(|arg| crate::mcp::McpPromptArgument {
                                    name: arg.name,
                                    description: arg.description.unwrap_or_default(),
                                    required: arg.required.unwrap_or(false),
                                })
                                .collect(),
                        })
                        .collect();

                    Ok(converted_prompts)
                }
                Err(e) => {
                    warn!("Failed to list prompts from rmcp peer {}: {}", self.name, e);
                    Err(Error::mcp(format!("Failed to list prompts: {}", e)))
                }
            }
        } else {
            warn!("rmcp peer not initialized for: {}", self.name);
            Ok(Vec::new())
        }
    }

    async fn get_prompt(
        &self,
        request: crate::mcp::McpGetPromptRequest,
    ) -> Result<crate::mcp::McpGetPromptResponse> {
        if let Some(ref peer) = self.peer {
            debug!(
                "Getting prompt '{}' from rmcp peer: {}",
                request.name, self.name
            );

            // Convert arguments to rmcp format
            let arguments = if request.arguments.is_empty() {
                None
            } else {
                Some(serde_json::Map::from_iter(
                    request
                        .arguments
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone())),
                ))
            };

            let rmcp_request = rmcp::model::GetPromptRequestParam {
                name: request.name.clone(),
                arguments,
            };

            match peer.get_prompt(rmcp_request).await {
                Ok(prompt_result) => {
                    debug!("Prompt '{}' retrieved successfully via rmcp", request.name);

                    // Convert rmcp prompt messages to our format
                    let messages = prompt_result
                        .messages
                        .into_iter()
                        .map(|message| crate::mcp::McpPromptMessage {
                            role: format!("{:?}", message.role).to_lowercase(),
                            content: match message.content {
                                rmcp::model::PromptMessageContent::Text { text } => {
                                    crate::mcp::McpContent::Text { text }
                                }
                                rmcp::model::PromptMessageContent::Image { image } => {
                                    crate::mcp::McpContent::Image {
                                        data: image.data.clone(),
                                        mime_type: image.mime_type.clone(),
                                    }
                                }
                                rmcp::model::PromptMessageContent::Resource { .. } => {
                                    // For now, convert resources to text representation
                                    crate::mcp::McpContent::Text {
                                        text: "Resource content (not yet supported)".to_string(),
                                    }
                                }
                            },
                        })
                        .collect();

                    Ok(crate::mcp::McpGetPromptResponse {
                        description: prompt_result.description.unwrap_or_default(),
                        messages,
                    })
                }
                Err(e) => {
                    warn!(
                        "Failed to get prompt '{}' via rmcp peer {}: {}",
                        request.name, self.name, e
                    );
                    Err(Error::mcp(format!("Get prompt failed: {}", e)))
                }
            }
        } else {
            warn!("rmcp peer not initialized for: {}", self.name);
            Err(Error::mcp("rmcp peer not initialized".to_string()))
        }
    }

    async fn close(&mut self) -> Result<()> {
        info!("Closing rmcp client: {}", self.name);

        if let Some(_peer) = self.peer.take() {
            // RunningService doesn't have a cancel method in the current rmcp version
            // The connection will be closed when the peer is dropped
            debug!("Successfully closed rmcp peer: {}", self.name);
        }

        Ok(())
    }
}

/// Factory function to create rmcp-based MCP client
pub async fn create_rmcp_client(config: McpServerConfig) -> Result<Box<dyn crate::mcp::McpClient>> {
    let client = RmcpClient::new(config).await?;
    Ok(Box::new(client))
}
