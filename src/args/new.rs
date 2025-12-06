use crate::prelude::{load_config, save_config};
use anyhow::Result;

// TODO: Decide on client/server config split for cloud deployment
// Current: Client modifies its own local ~/.blaze/config.toml and server reads (correct for same-machine setup)
// Future: Separate client config with server_url instead of sharing config
pub async fn new_run(source_name: String) -> Result<()> {
    println!("Creating a new source...");

    let mut config = load_config().await?;

    config.data_source.add_source(source_name);

    config.data_source.create_source_dir().await?;

    save_config(&config).await?;

    println!("Source created successfully!");

    Ok(())
}
