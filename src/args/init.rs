use crate::core::{Config, save_config};
use anyhow::Result;
use std::path::PathBuf;

pub async fn init_run(path: Option<PathBuf>) -> Result<()> {
    println!("Initializing Blaze-DB...");

    let config = match path {
        Some(p) => Config::create_config_at(p),
        None => Config::default(),
    };

    save_config(&config)?;

    println!(
        "Blaze-DB initialized successfully at {:?}",
        config.source_dir.path
    );

    Ok(())
}
