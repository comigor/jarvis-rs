use anyhow::Result;
use jarvis_rust::{config, server};
use tracing::info;

/// Validates that a log level string is valid
fn validate_log_level(level: &str) -> Result<()> {
    level
        .parse::<tracing_subscriber::filter::LevelFilter>()
        .map_err(|_| {
            anyhow::anyhow!(
                "Invalid log level: '{}'. Valid levels: error, warn, info, debug, trace",
                level
            )
        })?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration first (before logging setup)
    let config = match config::load().await {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Determine log level: environment variable overrides config
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| config.server.logs.level.clone());

    // Validate log level
    if let Err(e) = validate_log_level(&log_level) {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    // Initialize tracing with the determined log level
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.parse().unwrap()),
        )
        .json()
        .init();

    info!(
        "Starting J.A.R.V.I.S. Rust server with log level: {}",
        log_level
    );
    info!("Configuration loaded successfully");

    // Start the server
    server::run(config).await?;

    Ok(())
}
