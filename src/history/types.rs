use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Option<i64>,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(session_id: String, role: String, content: String) -> Self {
        Self {
            id: None,
            session_id,
            role,
            content,
            created_at: Utc::now(),
        }
    }

    pub fn user(session_id: String, content: String) -> Self {
        Self::new(session_id, "user".to_string(), content)
    }

    pub fn assistant(session_id: String, content: String) -> Self {
        Self::new(session_id, "assistant".to_string(), content)
    }

    pub fn system(session_id: String, content: String) -> Self {
        Self::new(session_id, "system".to_string(), content)
    }

    pub fn tool(session_id: String, content: String) -> Self {
        Self::new(session_id, "tool".to_string(), content)
    }
}