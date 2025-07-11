mod handlers;
mod types;

use crate::{agent::Agent, config::Config, history::HistoryStorage, Result};
use axum::{routing::post, Router};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use tracing::info;

pub async fn run(config: Config) -> Result<()> {
    // Initialize history storage
    let db_path = std::env::var("HISTORY_DB_PATH").unwrap_or_else(|_| config.server.database_path.clone());
    let history = HistoryStorage::new(&db_path).await?;
    
    // Initialize agent
    let agent = Agent::new(config.llm.clone(), config.mcp_servers.clone()).await?;
    
    // Create application state
    let app_state = handlers::AppState { 
        history: Arc::new(history), 
        agent: Arc::new(Mutex::new(agent))
    };
    
    // Create router
    let app = Router::new()
        .route("/", post(handlers::inference))
        .with_state(app_state);
    
    // Start server
    let addr = SocketAddr::new(
        config.server.host.parse()?,
        config.server.port,
    );
    
    info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}