use crate::core::{Config, save_config};
use anyhow::Result;

pub async fn init_run() -> Result<()> {
    println!("Initializing dotfiles...");

    let config = Config::default();
    save_config(&config).await?;

    Ok(())
}
