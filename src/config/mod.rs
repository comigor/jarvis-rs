mod types;

pub use types::*;

use crate::Result;
use std::env;
use tracing::debug;

pub async fn load() -> Result<Config> {
    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());

    debug!("Loading configuration from: {}", config_path);

    let config_str = tokio::fs::read_to_string(&config_path).await?;
    let config: Config = serde_yaml::from_str(&config_str)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const SAMPLE_CONFIG: &str = r#"
llm:
  provider: "openai"
  base_url: "https://api.openai.com"
  api_key: "test-key"
  model: "gpt-4"
  system_prompt: "You are a helpful assistant"

server:
  host: "127.0.0.1"
  port: 8080
  database_path: "test.db"
  logs:
    level: "debug"

mcp_servers:
  - name: "weather"
    type: "sse"
    url: "http://localhost:3000"
  - name: "file_manager"
    type: "stdio"
    command: "./file-server"
    args: ["--verbose"]
    env:
      DEBUG: "true"
"#;

    const MINIMAL_CONFIG: &str = r#"
llm:
  base_url: "https://api.openai.com"
  api_key: "test-key"
  model: "gpt-4"

server: {}
"#;

    const INVALID_CONFIG: &str = r#"
llm:
  # missing required fields
  
server:
  port: "not_a_number"
"#;

    #[tokio::test]
    async fn test_load_valid_config() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(SAMPLE_CONFIG.as_bytes()).unwrap();

        unsafe {
            env::set_var("CONFIG_PATH", temp_file.path().to_str().unwrap());
        }

        let config = load().await.unwrap();

        // Test LLM config
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.base_url, "https://api.openai.com");
        assert_eq!(config.llm.api_key, "test-key");
        assert_eq!(config.llm.model, "gpt-4");
        assert_eq!(
            config.llm.system_prompt,
            Some("You are a helpful assistant".to_string())
        );

        // Test server config
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.database_path, "test.db");
        assert_eq!(config.server.logs.level, "debug");

        // Test MCP servers
        assert_eq!(config.mcp_servers.len(), 2);

        let weather_server = &config.mcp_servers[0];
        assert_eq!(weather_server.name, "weather");
        assert!(matches!(weather_server.client_type, McpClientType::Sse));
        assert_eq!(
            weather_server.url,
            Some("http://localhost:3000".to_string())
        );

        let file_server = &config.mcp_servers[1];
        assert_eq!(file_server.name, "file_manager");
        assert!(matches!(file_server.client_type, McpClientType::Stdio));
        assert_eq!(file_server.command, Some("./file-server".to_string()));
        assert_eq!(file_server.args, vec!["--verbose"]);
        assert_eq!(file_server.env.get("DEBUG"), Some(&"true".to_string()));

        unsafe {
            env::remove_var("CONFIG_PATH");
        }
    }

    #[tokio::test]
    async fn test_load_minimal_config_with_defaults() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(MINIMAL_CONFIG.as_bytes()).unwrap();

        unsafe {
            env::set_var("CONFIG_PATH", temp_file.path().to_str().unwrap());
        }

        let config = load().await.unwrap();

        // Test defaults
        assert_eq!(config.llm.provider, "openai"); // default
        assert_eq!(config.server.host, "0.0.0.0"); // default
        assert_eq!(config.server.port, 8080); // default
        assert_eq!(config.server.logs.level, "info"); // default
        assert_eq!(config.server.database_path, "history.db"); // default
        assert_eq!(config.llm.system_prompt, None); // default
        assert!(config.mcp_servers.is_empty()); // default

        unsafe {
            env::remove_var("CONFIG_PATH");
        }
    }

    #[tokio::test]
    async fn test_load_missing_config_file() {
        unsafe {
            env::set_var("CONFIG_PATH", "/nonexistent/config.yaml");
        }

        let result = load().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("IO error"));

        unsafe {
            env::remove_var("CONFIG_PATH");
        }
    }

    #[tokio::test]
    async fn test_load_invalid_yaml() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"invalid: yaml: content: [").unwrap();

        unsafe {
            env::set_var("CONFIG_PATH", temp_file.path().to_str().unwrap());
        }

        let result = load().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("YAML error"));

        unsafe {
            env::remove_var("CONFIG_PATH");
        }
    }

    #[tokio::test]
    async fn test_load_invalid_config_structure() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(INVALID_CONFIG.as_bytes()).unwrap();

        unsafe {
            env::set_var("CONFIG_PATH", temp_file.path().to_str().unwrap());
        }

        let result = load().await;
        assert!(result.is_err());

        unsafe {
            env::remove_var("CONFIG_PATH");
        }
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = Config {
            llm: LlmConfig {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com".to_string(),
                api_key: "test-key".to_string(),
                model: "gpt-4".to_string(),
                system_prompt: Some("Test prompt".to_string()),
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                database_path: "test.db".to_string(),
                logs: LogsConfig {
                    level: "debug".to_string(),
                },
            },
            mcp_servers: vec![McpServerConfig {
                name: "test".to_string(),
                url: Some("http://localhost:3000".to_string()),
                client_type: McpClientType::Sse,
                headers: std::collections::HashMap::new(),
                command: None,
                args: vec![],
                env: std::collections::HashMap::new(),
            }],
        };

        // Test serialization
        let yaml_str = serde_yaml::to_string(&config).unwrap();
        assert!(yaml_str.contains("openai"));
        assert!(yaml_str.contains("127.0.0.1"));
        assert!(yaml_str.contains("test"));

        // Test deserialization
        let deserialized: Config = serde_yaml::from_str(&yaml_str).unwrap();
        assert_eq!(config.llm.provider, deserialized.llm.provider);
        assert_eq!(config.server.host, deserialized.server.host);
        assert_eq!(config.mcp_servers.len(), deserialized.mcp_servers.len());
    }

    #[test]
    fn test_mcp_client_type_serialization() {
        assert_eq!(
            serde_yaml::to_string(&McpClientType::Sse).unwrap().trim(),
            "sse"
        );
        assert_eq!(
            serde_yaml::to_string(&McpClientType::StreamableHttp)
                .unwrap()
                .trim(),
            "streamable_http"
        );
        assert_eq!(
            serde_yaml::to_string(&McpClientType::Stdio).unwrap().trim(),
            "stdio"
        );

        // Test deserialization
        assert!(matches!(
            serde_yaml::from_str::<McpClientType>("sse").unwrap(),
            McpClientType::Sse
        ));
        assert!(matches!(
            serde_yaml::from_str::<McpClientType>("streamable_http").unwrap(),
            McpClientType::StreamableHttp
        ));
        assert!(matches!(
            serde_yaml::from_str::<McpClientType>("stdio").unwrap(),
            McpClientType::Stdio
        ));
    }

    #[test]
    fn test_default_values() {
        let logs_config = LogsConfig::default();
        assert_eq!(logs_config.level, "info");

        // Test default functions
        assert_eq!(types::default_provider(), "openai");
        assert_eq!(types::default_host(), "0.0.0.0");
        assert_eq!(types::default_port(), 8080);
        assert_eq!(types::default_log_level(), "info");
        assert_eq!(types::default_database_path(), "history.db");
    }
}
