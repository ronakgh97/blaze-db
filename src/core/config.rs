use std::fs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub source_dir: Source,
}

#[derive(Serialize, Deserialize)]
pub struct Source {
    pub path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source_dir: Source {
                path: dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("blaze-db")
                    .join("source"),
            },
        }
    }
}

impl Config {
    pub fn create_config_at(path: PathBuf) -> Self {
        Self {
            source_dir: Source { path },
        }
    }
}

pub fn create_source_dir(config: &Config) -> Result<()> {
    let source_path = &config.source_dir.path;
    if !source_path.exists() {
        fs::create_dir_all(source_path).with_context(|| {
            format!(
                "Failed to create source directory at {}",
                source_path.display()
            )
        })?;
    }
    Ok(())
}

/// Save config to default location
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;

    // Create parent directory
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }

    let toml_string = toml::to_string_pretty(config)
        .with_context(|| format!("Failed to serialize config to {}", config_path.display()))?;

    fs::write(&config_path, toml_string)
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

    Ok(())
}

/// Load config from default location
pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;

    let config_content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file {}", config_path.display()))?;

    let config: Config =
        toml::from_str(&config_content).with_context(|| "Failed to parse config".to_string())?;

    Ok(config)
}

/// Get default config path
fn get_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().with_context(|| "No home directory?")?;
    Ok(home.join(".rshare").join("config.toml"))
}
