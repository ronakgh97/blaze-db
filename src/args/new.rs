use crate::core::ServerConfig;
use crate::prelude::save_config;
use anyhow::Result;

pub async fn new_run(source_name: String) -> Result<()> {
    println!("Creating a new source...");

    let mut config =
        ServerConfig::load_config(&ServerConfig::get_default_server_config_path()?).await?;

    config.data_source.add_source(source_name.clone())?;

    config.data_source.create_source_dir().await?;

    save_config(ServerConfig::get_default_server_config_path()?, &config).await?;

    println!("Source: [`{}`] created successfully!", source_name);

    Ok(())
}
