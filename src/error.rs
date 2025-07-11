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
                requested: requested.clone() 
            },
            Self::MaxTurnsExceeded { max_turns } => Self::MaxTurnsExceeded { max_turns: *max_turns },
            Self::ToolNotFound { tool_name } => Self::ToolNotFound { tool_name: tool_name.clone() },
            Self::SessionNotFound { session_id } => Self::SessionNotFound { session_id: session_id.clone() },
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