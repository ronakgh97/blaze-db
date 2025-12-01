use crate::prelude::load_config;
use anyhow::Result;
use tokio::fs::create_dir_all;

pub async fn create_run(name: String, dimensions: usize) -> Result<()> {
    println!("Creating a new database...");

    validate_input(name.clone(), dimensions).await?;

    let config = load_config()?;
    let dir_name = format!("{}_{}", name, dimensions);
    let db_path = config.source_dir.path.join(&dir_name);

    create_dir_all(&db_path).await?;

    Ok(())
}

async fn validate_input(name: String, dimensions: usize) -> Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("Database name cannot be empty");
    }

    if dimensions == 0 {
        anyhow::bail!("Dimensions must be greater than zero");
    }

    Ok(())
}
