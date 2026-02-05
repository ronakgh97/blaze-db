use crate::core::UserConfig;
use crate::server::{
    CreateDatabaseRequest, CreateDatabaseResponse, CreateSourceRequest, CreateSourceResponse,
};
use anyhow::Result;
use colored::Colorize;

// Create either a new source and database, or just a new database within an existing source, based on option args
pub async fn create_run(
    name: Option<String>,
    src: String,
    dimensions: Option<usize>,
) -> Result<()> {
    let dim = dimensions.unwrap_or(1024);

    // I don't care, lets server handle whatever it can.

    if name.is_none() {
        // Create source only
        let _ = create_source(&src).await;
        Ok(())
    } else {
        let _ = create_source(&src).await;
        create_database(name.unwrap(), &src, dim).await?;
        Ok(())
    }
}

async fn create_database(name: String, src: &String, dimensions: usize) -> Result<()> {
    println!("Creating a new database: {}", name.yellow());

    let config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    let request_body = CreateDatabaseRequest {
        name,
        source: src.clone(),
        dimensions,
    };

    dotenv::dotenv().ok();
    let api_key = std::env::var("BLAZE_API_KEY").unwrap_or("local_dev_key".to_string());

    let response = reqwest::Client::new()
        .post(config.server.instance_url + "/v1/blazedb/databases/create")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_json: CreateDatabaseResponse = response.json().await?;
        println!(
            "Database ({}) created with ID: {}, Dimensions: {}",
            resp_json.name.yellow(),
            resp_json.id.to_string().cyan().dimmed(),
            resp_json.dimensions.to_string().cyan().dimmed()
        );
        Ok(())
    } else {
        anyhow::bail!("Failed to create database. Status: {}", response.status())
    }
}

async fn create_source(name: &String) -> Result<()> {
    println!("Creating a new source: {}", name.yellow());

    let config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    let request_body = CreateSourceRequest {
        source_name: name.to_string(),
    };

    dotenv::dotenv().ok();
    let api_key = std::env::var("BLAZE_API_KEY").unwrap_or("local_dev_key".to_string());

    let response = reqwest::Client::new()
        .post(config.server.instance_url + "/v1/blazedb/sources/create")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_json: CreateSourceResponse = response.json().await?;
        println!(
            "Source ({}) created with ID: {}",
            resp_json.source.yellow(),
            resp_json.id.to_string().cyan().dimmed()
        );
        Ok(())
    } else {
        anyhow::bail!("Failed to create source. Status: {}", response.status())
    }
}
