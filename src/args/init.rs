use crate::core::{ClientConfig, ServerConfig, save_config};
use anyhow::Result;

pub async fn init_run_server() -> Result<()> {
    println!("Initializing dotfiles...");

    let config = ServerConfig::default();
    save_config(ServerConfig::get_default_server_config_path()?, &config).await?;

    Ok(())
}

pub async fn init_run_client(url: Option<String>) -> Result<()> {
    println!("Initializing dotfiles...");

    let mut config = ClientConfig::default();
    if let Some(url) = url {
        config.update(url, config.timeout);
    }

    save_config(ClientConfig::get_default_user_config_path()?, &config).await?;

    Ok(())
}
