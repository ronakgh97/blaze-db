use crate::prelude::{load_config, save_config};
use anyhow::Result;
use std::path::PathBuf;

pub async fn new_run(path: PathBuf) -> Result<()> {
    println!("Creating a new source...");

    let mut config = load_config().await?;

    let source = config.data_source.add_source_path(path).await;

    source.create_source_dir().await?;

    config.data_source = source;

    save_config(&config).await?;

    Ok(())
}
