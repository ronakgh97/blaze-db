use crate::prelude::Source;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientConfig {
    pub url: String,
    pub timeout: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080".to_string(),
            timeout: 30,
        }
    }
}

impl ClientConfig {
    pub fn new(url: String, timeout: u64) -> Self {
        Self { url, timeout }
    }

    pub fn update(&mut self, url: String, timeout: u64) {
        self.url = url;
        self.timeout = timeout;
    }

    /// Load client config from given location
    pub async fn load_config(config_path: &PathBuf) -> Result<ClientConfig> {
        let config_content = fs::read_to_string(&config_path)
            .await
            .with_context(|| format!("Failed to read config file {}", config_path.display()))?;

        let config: ClientConfig = toml::from_str(&config_content)
            .with_context(|| "Failed to parse config".to_string())?;

        Ok(config)
    }

    /// Get default config path
    pub fn get_default_user_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().with_context(|| "No home directory?")?;
        Ok(home.join(".config").join("blaze").join("user_config.toml"))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub server_connection: ConnectionConfig,
    pub data_source: Source,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionConfig {
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_connection: ConnectionConfig { port: 8080 },
            data_source: { Source { source_name: None } },
        }
    }
}

impl ServerConfig {
    /// Get the data source object from config
    pub fn get_source(&self) -> Result<Source> {
        Ok(self.data_source.clone())
    }

    /// Update the data source in config
    pub fn update_source(&mut self, source: Source) {
        self.data_source = source;
    }

    /// Load server config from given location
    pub async fn load_config(config_path: &PathBuf) -> Result<ServerConfig> {
        let config_content = fs::read_to_string(&config_path)
            .await
            .with_context(|| format!("Failed to read config file {}", config_path.display()))?;

        let config: ServerConfig = toml::from_str(&config_content)
            .with_context(|| "Failed to parse config".to_string())?;

        Ok(config)
    }

    /// Get default server config path
    pub fn get_default_server_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().with_context(|| "No home directory?")?;
        Ok(home
            .join(".config")
            .join("blaze")
            .join("server_config.toml"))
    }
}

/// Save configs
pub async fn save_config<T>(config_path: PathBuf, config: &T) -> Result<()>
where
    T: Serialize,
{
    // Create parent directory
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }

    let toml_string = toml::to_string_pretty(&config)
        .with_context(|| format!("Failed to serialize config to {}", config_path.display()))?;

    fs::write(&config_path, toml_string)
        .await
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

    Ok(())
}
