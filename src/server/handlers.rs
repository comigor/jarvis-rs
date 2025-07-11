use super::types::{ErrorResponse, InferenceRequest, InferenceResponse};
use crate::{agent::Agent, history::HistoryStorage};
use axum::{extract::State, http::StatusCode, response::Json};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub history: Arc<HistoryStorage>,
    pub agent: Arc<Mutex<Agent>>,
}

pub async fn inference(
    State(state): State<AppState>,
    Json(request): Json<InferenceRequest>,
) -> Result<Json<InferenceResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Received inference request for input: {}", request.input);

    // Generate session ID if not provided
    let session_id = request
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Process the request through the agent
    let mut agent = state.agent.lock().await;
    match agent
        .process(&session_id, &request.input, &state.history)
        .await
    {
        Ok(output) => {
            info!("Successfully processed request for session: {}", session_id);
            Ok(Json(InferenceResponse { session_id, output }))
        }
        Err(e) => {
            error!(
                "Failed to process request for session {}: {}",
                session_id, e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Processing error: {}", e),
                }),
            ))
        }
    }
}
