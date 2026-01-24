use crate::core::{ClientConfig, ServerConfig, get_source_path, save_config};
use anyhow::Result;

pub async fn init_run_server() -> Result<()> {
    println!("Initializing dotfiles/src...");

    let mut config = ServerConfig::default();

    let mut get_source = config.get_source().clone();

    // Check if default source and default config already exists to avoid overwriting
    if get_source_path()?.join("default_src").exists()
        && ServerConfig::get_default_server_config_path()?.exists()
    {
        println!("Defaults already initialized at {:?}", get_source_path()?);
        return Ok(());
    }

    get_source.add_source("default_src".to_string())?; // Add default source
    get_source.create_source_dir().await?;

    // Update config with the modified source
    config.update_source(get_source);

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
