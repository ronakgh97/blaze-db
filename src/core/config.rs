use crate::prelude::Source;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub data_source: Source,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_source: { Source { source_name: None } },
        }
    }
}

impl Config {
    /// Get the data source object from config
    pub fn get_source_object(&self) -> Result<Source> {
        Ok(self.data_source.clone())
    }

    /// Update the data source in config
    pub fn update_source(&mut self, source: Source) {
        self.data_source = source;
    }
}

/// Save config to default location
pub async fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;

    // Create parent directory
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }

    let toml_string = toml::to_string_pretty(config)
        .with_context(|| format!("Failed to serialize config to {}", config_path.display()))?;

    fs::write(&config_path, toml_string)
        .await
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

    Ok(())
}

/// Load config from default location
pub async fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;

    let config_content = fs::read_to_string(&config_path)
        .await
        .with_context(|| format!("Failed to read config file {}", config_path.display()))?;

    let config: Config =
        toml::from_str(&config_content).with_context(|| "Failed to parse config".to_string())?;

    Ok(config)
}

/// Get default config path
fn get_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().with_context(|| "No home directory?")?;
    Ok(home.join(".blaze").join("config.toml"))
}
