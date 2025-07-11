use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub logs: LogsConfig,
    #[serde(default = "default_database_path")]
    pub database_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub client_type: McpClientType,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    // Stdio specific fields
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpClientType {
    Sse,
    StreamableHttp,
    Stdio,
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_database_path() -> String {
    "history.db".to_string()
}