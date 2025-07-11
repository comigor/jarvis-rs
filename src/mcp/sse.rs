use super::client::*;
use crate::{config::McpServerConfig, Error, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, warn};

pub struct SseMcpClient {
    base_url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
    initialized: bool,
    name: String,
}

impl SseMcpClient {
    pub async fn new(config: McpServerConfig) -> Result<Self> {
        let base_url = config.url.ok_or_else(|| {
            Error::mcp("SSE MCP client requires URL field")
        })?;

        debug!("Creating SSE MCP client for: {}", config.name);

        let client = reqwest::Client::new();

        Ok(Self {
            base_url,
            headers: config.headers,
            client,
            initialized: false,
            name: config.name,
        })
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let mut req_builder = self.client
            .post(&self.base_url)
            .json(&request);

        // Add custom headers
        for (key, value) in &self.headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder.send().await.map_err(|e| {
            Error::mcp(format!("Failed to send SSE MCP request: {}", e))
        })?;

        let response_json: Value = response.json().await.map_err(|e| {
            Error::mcp(format!("Failed to parse SSE MCP response: {}", e))
        })?;

        // Check for JSON-RPC error
        if let Some(error) = response_json.get("error") {
            return Err(Error::mcp(format!("SSE MCP error: {}", error)));
        }

        response_json.get("result").cloned().ok_or_else(|| {
            Error::mcp("SSE MCP response missing result field")
        })
    }
}

#[async_trait]
impl McpClient for SseMcpClient {
    async fn initialize(&mut self, request: McpInitializeRequest) -> Result<McpInitializeResponse> {
        debug!("Initializing SSE MCP client: {}", self.name);
        
        let params = serde_json::to_value(request)?;
        let result = self.send_request("initialize", params).await?;
        
        let response: McpInitializeResponse = serde_json::from_value(result)?;
        self.initialized = true;
        
        debug!("Successfully initialized SSE MCP client: {}", self.name);
        Ok(response)
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!("Listing tools for SSE MCP client: {}", self.name);
        
        let result = self.send_request("tools/list", json!({})).await?;
        let tools_response: Value = result;
        
        let tools = tools_response.get("tools")
            .and_then(|t| t.as_array())
            .ok_or_else(|| Error::mcp("Invalid tools list response"))?;

        let mut mcp_tools = Vec::new();
        for tool in tools {
            let mcp_tool: McpTool = serde_json::from_value(tool.clone())?;
            mcp_tools.push(mcp_tool);
        }

        debug!("Found {} tools for SSE MCP client: {}", mcp_tools.len(), self.name);
        Ok(mcp_tools)
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> Result<McpToolCallResponse> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!("Calling tool '{}' for SSE MCP client: {}", request.name, self.name);
        
        let params = serde_json::to_value(request)?;
        let result = self.send_request("tools/call", params).await?;
        
        let response: McpToolCallResponse = serde_json::from_value(result)?;
        Ok(response)
    }

    async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!("Listing prompts for SSE MCP client: {}", self.name);
        
        let result = self.send_request("prompts/list", json!({})).await?;
        let prompts_response: Value = result;
        
        let prompts = prompts_response.get("prompts")
            .and_then(|p| p.as_array())
            .ok_or_else(|| Error::mcp("Invalid prompts list response"))?;

        let mut mcp_prompts = Vec::new();
        for prompt in prompts {
            let mcp_prompt: McpPrompt = serde_json::from_value(prompt.clone())?;
            mcp_prompts.push(mcp_prompt);
        }

        debug!("Found {} prompts for SSE MCP client: {}", mcp_prompts.len(), self.name);
        Ok(mcp_prompts)
    }

    async fn get_prompt(&self, request: McpGetPromptRequest) -> Result<McpGetPromptResponse> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!("Getting prompt '{}' for SSE MCP client: {}", request.name, self.name);
        
        let params = serde_json::to_value(request)?;
        let result = self.send_request("prompts/get", params).await?;
        
        let response: McpGetPromptResponse = serde_json::from_value(result)?;
        Ok(response)
    }

    async fn close(&mut self) -> Result<()> {
        debug!("Closing SSE MCP client: {}", self.name);
        // Nothing specific to close for HTTP-based client
        Ok(())
    }
}