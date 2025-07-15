use jarvis_rust::config::{Config, LlmConfig, LogsConfig, ServerConfig};

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
