use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] libsql::Error),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("FSM error: {0}")]
    Fsm(String),

    #[error("HTTP error: {0}")]
    Http(#[from] axum::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Address parse error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("OpenAI error: {0}")]
    OpenAi(#[from] async_openai::error::OpenAIError),

    #[error("Invalid state transition: {current} -> {requested}")]
    InvalidTransition { current: String, requested: String },

    #[error("Max interaction turns exceeded: {max_turns}")]
    MaxTurnsExceeded { max_turns: usize },

    #[error("Tool not found: {tool_name}")]
    ToolNotFound { tool_name: String },

    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Self::Config(s) => Self::Config(s.clone()),
            Self::Llm(s) => Self::Llm(s.clone()),
            Self::Mcp(s) => Self::Mcp(s.clone()),
            Self::Fsm(s) => Self::Fsm(s.clone()),
            Self::InvalidTransition { current, requested } => Self::InvalidTransition {
                current: current.clone(),
                requested: requested.clone(),
            },
            Self::MaxTurnsExceeded { max_turns } => Self::MaxTurnsExceeded {
                max_turns: *max_turns,
            },
            Self::ToolNotFound { tool_name } => Self::ToolNotFound {
                tool_name: tool_name.clone(),
            },
            Self::SessionNotFound { session_id } => Self::SessionNotFound {
                session_id: session_id.clone(),
            },
            Self::Internal(s) => Self::Internal(s.clone()),
            // For errors that can't be cloned, convert to string representation
            Self::Database(e) => Self::Internal(format!("Database error: {}", e)),
            Self::Http(e) => Self::Internal(format!("HTTP error: {}", e)),
            Self::Serialization(e) => Self::Internal(format!("Serialization error: {}", e)),
            Self::Yaml(e) => Self::Internal(format!("YAML error: {}", e)),
            Self::Io(e) => Self::Internal(format!("IO error: {}", e)),
            Self::Network(e) => Self::Internal(format!("Network error: {}", e)),
            Self::AddrParse(e) => Self::Internal(format!("Address parse error: {}", e)),
            Self::Uuid(e) => Self::Internal(format!("UUID error: {}", e)),
            Self::OpenAi(e) => Self::Internal(format!("OpenAI error: {}", e)),
        }
    }
}

impl Error {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn llm(msg: impl Into<String>) -> Self {
        Self::Llm(msg.into())
    }

    pub fn mcp(msg: impl Into<String>) -> Self {
        Self::Mcp(msg.into())
    }

    pub fn fsm(msg: impl Into<String>) -> Self {
        Self::Fsm(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_construction() {
        let config_err = Error::config("Invalid config");
        assert_eq!(config_err.to_string(), "Configuration error: Invalid config");

        let llm_err = Error::llm("LLM failed");
        assert_eq!(llm_err.to_string(), "LLM error: LLM failed");

        let mcp_err = Error::mcp("MCP connection failed");
        assert_eq!(mcp_err.to_string(), "MCP error: MCP connection failed");

        let fsm_err = Error::fsm("Invalid state transition");
        assert_eq!(fsm_err.to_string(), "FSM error: Invalid state transition");

        let internal_err = Error::internal("Internal server error");
        assert_eq!(internal_err.to_string(), "Internal error: Internal server error");
    }

    #[test]
    fn test_error_variants() {
        let invalid_transition = Error::InvalidTransition {
            current: "Ready".to_string(),
            requested: "Invalid".to_string(),
        };
        assert_eq!(
            invalid_transition.to_string(),
            "Invalid state transition: Ready -> Invalid"
        );

        let max_turns = Error::MaxTurnsExceeded { max_turns: 5 };
        assert_eq!(max_turns.to_string(), "Max interaction turns exceeded: 5");

        let tool_not_found = Error::ToolNotFound {
            tool_name: "weather_tool".to_string(),
        };
        assert_eq!(tool_not_found.to_string(), "Tool not found: weather_tool");

        let session_not_found = Error::SessionNotFound {
            session_id: "session-123".to_string(),
        };
        assert_eq!(session_not_found.to_string(), "Session not found: session-123");
    }

    #[test]
    fn test_error_from_conversions() {
        // Test std::io::Error conversion
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let jarvis_error: Error = io_error.into();
        assert!(jarvis_error.to_string().contains("IO error"));

        // Test serde_json::Error conversion
        let json_error: serde_json::Error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let jarvis_error: Error = json_error.into();
        assert!(jarvis_error.to_string().contains("Serialization error"));

        // Test std::net::AddrParseError conversion
        let addr_error = "invalid-address".parse::<std::net::IpAddr>().unwrap_err();
        let jarvis_error: Error = addr_error.into();
        assert!(jarvis_error.to_string().contains("Address parse error"));
    }

    #[test]
    fn test_error_clone() {
        let original = Error::config("Test config error");
        let cloned = original.clone();
        assert_eq!(original.to_string(), cloned.to_string());

        let original = Error::InvalidTransition {
            current: "State1".to_string(),
            requested: "State2".to_string(),
        };
        let cloned = original.clone();
        assert_eq!(original.to_string(), cloned.to_string());

        let original = Error::MaxTurnsExceeded { max_turns: 10 };
        let cloned = original.clone();
        assert_eq!(original.to_string(), cloned.to_string());
    }

    #[test]
    fn test_error_clone_with_non_cloneable_types() {
        // Test cloning errors that can't be directly cloned
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied");
        let jarvis_error: Error = io_error.into();
        let cloned = jarvis_error.clone();
        
        // Should be converted to Internal error with string representation
        assert!(cloned.to_string().contains("Internal error"));
        assert!(cloned.to_string().contains("IO error"));
    }

    #[test]
    fn test_result_type() {
        fn test_function() -> Result<String> {
            Ok("success".to_string())
        }

        fn test_error_function() -> Result<String> {
            Err(Error::internal("test error"))
        }

        assert!(test_function().is_ok());
        assert!(test_error_function().is_err());
    }
}
