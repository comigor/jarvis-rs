use super::client::*;
use crate::{config::McpServerConfig, Error, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub struct StdioMcpClient {
    process: Mutex<Option<Child>>,
    initialized: bool,
    name: String,
}

impl StdioMcpClient {
    pub async fn new(config: McpServerConfig) -> Result<Self> {
        let command = config
            .command
            .ok_or_else(|| Error::mcp("stdio MCP client requires command field"))?;

        debug!("Creating stdio MCP client for: {}", config.name);

        let mut cmd = Command::new(&command);
        cmd.args(&config.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let process = cmd.spawn().map_err(|e| {
            Error::mcp(format!(
                "Failed to spawn MCP stdio process {}: {}",
                command, e
            ))
        })?;

        Ok(Self {
            process: Mutex::new(Some(process)),
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

        let mut process_guard = self.process.lock().await;
        let process = process_guard
            .as_mut()
            .ok_or_else(|| Error::mcp("MCP stdio process not available"))?;

        let stdin = process
            .stdin
            .as_mut()
            .ok_or_else(|| Error::mcp("Failed to get stdin for MCP stdio process"))?;

        let stdout = process
            .stdout
            .as_mut()
            .ok_or_else(|| Error::mcp("Failed to get stdout for MCP stdio process"))?;

        // Send request
        let request_line = format!("{}\n", serde_json::to_string(&request)?);
        stdin
            .write_all(request_line.as_bytes())
            .await
            .map_err(|e| Error::mcp(format!("Failed to write to MCP stdio process: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| Error::mcp(format!("Failed to flush MCP stdio process stdin: {}", e)))?;

        // Read response
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| Error::mcp(format!("Failed to read from MCP stdio process: {}", e)))?;

        let response: Value = serde_json::from_str(&response_line)
            .map_err(|e| Error::mcp(format!("Failed to parse MCP stdio response: {}", e)))?;

        // Check for JSON-RPC error
        if let Some(error) = response.get("error") {
            return Err(Error::mcp(format!("MCP stdio error: {}", error)));
        }

        response
            .get("result")
            .cloned()
            .ok_or_else(|| Error::mcp("MCP stdio response missing result field"))
    }
}

#[async_trait]
impl McpClient for StdioMcpClient {
    async fn initialize(&mut self, request: McpInitializeRequest) -> Result<McpInitializeResponse> {
        debug!("Initializing stdio MCP client: {}", self.name);

        let params = serde_json::to_value(request)?;
        let result = self.send_request("initialize", params).await?;

        let response: McpInitializeResponse = serde_json::from_value(result)?;
        self.initialized = true;

        debug!("Successfully initialized stdio MCP client: {}", self.name);
        Ok(response)
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!("Listing tools for stdio MCP client: {}", self.name);

        let result = self.send_request("tools/list", json!({})).await?;
        let tools_response: Value = result;

        let tools = tools_response
            .get("tools")
            .and_then(|t| t.as_array())
            .ok_or_else(|| Error::mcp("Invalid tools list response"))?;

        let mut mcp_tools = Vec::new();
        for tool in tools {
            let mcp_tool: McpTool = serde_json::from_value(tool.clone())?;
            mcp_tools.push(mcp_tool);
        }

        debug!(
            "Found {} tools for stdio MCP client: {}",
            mcp_tools.len(),
            self.name
        );
        Ok(mcp_tools)
    }

    async fn call_tool(&self, request: McpToolCallRequest) -> Result<McpToolCallResponse> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!(
            "Calling tool '{}' for stdio MCP client: {}",
            request.name, self.name
        );

        let params = serde_json::to_value(request)?;
        let result = self.send_request("tools/call", params).await?;

        let response: McpToolCallResponse = serde_json::from_value(result)?;
        Ok(response)
    }

    async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!("Listing prompts for stdio MCP client: {}", self.name);

        let result = self.send_request("prompts/list", json!({})).await?;
        let prompts_response: Value = result;

        let prompts = prompts_response
            .get("prompts")
            .and_then(|p| p.as_array())
            .ok_or_else(|| Error::mcp("Invalid prompts list response"))?;

        let mut mcp_prompts = Vec::new();
        for prompt in prompts {
            let mcp_prompt: McpPrompt = serde_json::from_value(prompt.clone())?;
            mcp_prompts.push(mcp_prompt);
        }

        debug!(
            "Found {} prompts for stdio MCP client: {}",
            mcp_prompts.len(),
            self.name
        );
        Ok(mcp_prompts)
    }

    async fn get_prompt(&self, request: McpGetPromptRequest) -> Result<McpGetPromptResponse> {
        if !self.initialized {
            return Err(Error::mcp("MCP client not initialized"));
        }

        debug!(
            "Getting prompt '{}' for stdio MCP client: {}",
            request.name, self.name
        );

        let params = serde_json::to_value(request)?;
        let result = self.send_request("prompts/get", params).await?;

        let response: McpGetPromptResponse = serde_json::from_value(result)?;
        Ok(response)
    }

    async fn close(&mut self) -> Result<()> {
        debug!("Closing stdio MCP client: {}", self.name);

        let mut process_guard = self.process.lock().await;
        if let Some(mut process) = process_guard.take() {
            if let Err(e) = process.kill().await {
                warn!("Failed to kill MCP stdio process {}: {}", self.name, e);
            }
        }

        Ok(())
    }
}
