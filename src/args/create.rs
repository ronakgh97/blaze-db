use anyhow::Result;

pub async fn create_run(name: String, dimensions: usize) -> Result<()> {
    println!("Creating a new database...");

    validate_input(name.clone(), dimensions).await?;

    //TODO: Implement database creation with selected source logic here

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
