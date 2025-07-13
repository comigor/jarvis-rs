use jarvis_rust::{
    Result,
    config::{Config, LlmConfig, LogsConfig, McpServerConfig, ServerConfig},
    history::HistoryStorage,
    mcp::McpClientType,
};
use serde_json::Value;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::fs;

/// Create a test configuration with sensible defaults
pub fn create_test_config() -> Config {
    Config {
        server: ServerConfig {
            host: "127.0.0.1".parse().unwrap(),
            port: 8080,
            database_path: ":memory:".to_string(),
            logs: LogsConfig {
                level: "debug".to_string(),
            },
        },
        llm: LlmConfig {
            provider: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            api_key: "test-api-key".to_string(),
            model: "gpt-4".to_string(),
            system_prompt: Some("You are a helpful assistant.".to_string()),
        },
        mcp_servers: vec![],
    }
}

/// Create a test configuration with MCP servers
pub fn create_test_config_with_mcp() -> Config {
    let mut config = create_test_config();
    config.mcp_servers = vec![McpServerConfig {
        name: "test-server".to_string(),
        client_type: McpClientType::Http,
        url: Some("http://localhost:3000".to_string()),
        command: None,
        args: Vec::new(),
        env: HashMap::new(),
        headers: HashMap::new(),
    }];
    config
}

/// Create a temporary directory for test files
pub fn create_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

/// Create a test config YAML file
pub async fn create_test_config_file(dir: &TempDir, content: &str) -> Result<String> {
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, content).await?;
    Ok(config_path.to_string_lossy().to_string())
}

/// Create a temporary database for testing
pub async fn create_test_db() -> Result<(TempDir, HistoryStorage)> {
    let temp_dir = create_temp_dir();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    let storage = HistoryStorage::new(&db_path_str).await?;
    Ok((temp_dir, storage))
}

/// Create an in-memory database for testing
pub async fn create_in_memory_db() -> Result<HistoryStorage> {
    HistoryStorage::new(":memory:").await
}

/// Sample configuration YAML for testing
pub const SAMPLE_CONFIG_YAML: &str = r#"
server:
  host: "127.0.0.1"
  port: 8080
  database_path: ":memory:"
  logs:
    level: "debug"

llm:
  provider: "openai"
  base_url: "https://api.openai.com"
  api_key: "test-api-key"
  model: "gpt-4"
  system_prompt: "You are a helpful assistant."

mcp_servers:
  - name: "test-server"
    type: "http"
    url: "http://localhost:3000"
"#;

/// Sample configuration with stdio MCP server
pub const SAMPLE_CONFIG_WITH_STDIO: &str = r#"
server:
  host: "127.0.0.1"
  port: 8080
  database_path: ":memory:"
  logs:
    level: "info"

llm:
  provider: "openai"
  base_url: "https://api.openai.com"
  api_key: "test-api-key"
  model: "gpt-4"

mcp_servers:
  - name: "stdio-server"
    type: "stdio"
    command: "./mock-server"
    args: ["--verbose"]
    env:
      DEBUG: "true"
"#;

/// Sample configuration with SSE MCP server
pub const SAMPLE_CONFIG_WITH_SSE: &str = r#"
server:
  host: "127.0.0.1"
  port: 8080
  database_path: "/tmp/test.db"
  logs:
    level: "warn"

llm:
  provider: "openai"
  base_url: "https://api.openai.com"
  api_key: "test-api-key"
  model: "gpt-3.5-turbo"

mcp_servers:
  - name: "sse-server"
    type: "sse"
    url: "http://localhost:4000/sse"
"#;

/// Invalid configuration YAML for testing error cases
pub const INVALID_CONFIG_YAML: &str = r#"
server:
  host: "invalid-host"
  port: "not-a-number"

llm:
  provider: "unknown"
  # missing required fields

mcp_servers:
  - name: ""  # invalid empty name
    type: "invalid-type"
"#;

/// Assertion helpers for test results
pub fn assert_contains_error(result: &Result<String>, expected_error: &str) {
    match result {
        Err(e) => assert!(
            e.to_string().contains(expected_error),
            "Expected error containing '{}', got: {}",
            expected_error,
            e
        ),
        Ok(content) => panic!(
            "Expected error containing '{}', but got success: {}",
            expected_error, content
        ),
    }
}

/// Create test arguments for MCP tool calls
pub fn create_test_arguments(key: &str, value: &str) -> HashMap<String, Value> {
    let mut args = HashMap::new();
    args.insert(key.to_string(), Value::String(value.to_string()));
    args
}

/// Generate unique session ID for tests
pub fn generate_test_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
