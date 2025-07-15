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
