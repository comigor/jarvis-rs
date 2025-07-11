use anyhow::Result;
use jarvis_rust::{config, server};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    info!("Starting J.A.R.V.I.S. Rust server");

    // Load configuration
    let config = config::load().await?;
    info!("Configuration loaded successfully");

    // Start the server
    server::run(config).await?;

    Ok(())
}
