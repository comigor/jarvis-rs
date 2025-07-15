use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct InferenceRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    pub input: String,
}

#[derive(Debug, Serialize)]
pub struct InferenceResponse {
    pub session_id: String,
    pub output: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
