use crate::prelude::{load_config, save_config};
use anyhow::Result;

pub async fn new_run(source_name: String) -> Result<()> {
    println!("Creating a new source...");

    let mut config = load_config().await?;

    config.data_source.add_source(source_name);

    config.data_source.create_source_dir().await?;

    save_config(&config).await?;

    println!("Source created successfully!");

    Ok(())
}
