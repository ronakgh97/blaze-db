use crate::core::ClientConfig;
use crate::server::{
    CreateDatabaseRequest, CreateDatabaseResponse, CreateSourceRequest, CreateSourceResponse,
    list_databases,
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

    if name.is_none() {
        // Create source only
        create_source(&src).await?;
        Ok(())
    } else {
        // TODO: FUCKKKKKKKKKKKKKKKKKKKKK THIS IS UGLY AS HELL, IM SO FUCKING ANGRY AND SAD TODAY
        // TODO: FIX THIS LATER, THIS SHOULD A SERVER SIDE CHECK
        // Try to create source first (server will skip if it exists)
        let _ = create_source(&src).await; // Ignore error if source already exists
        // Create database within source
        create_database(name.unwrap(), &src, dim).await?;
        Ok(())
    }
}

async fn create_database(name: String, src: &String, dimensions: usize) -> Result<()> {
    println!("Creating a new database: {}", name.yellow());

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let request_body = CreateDatabaseRequest {
        name,
        source: src.clone(),
        dimensions,
    };

    // TODO: ALL Checks should be done server-side, fix this later

    // Check for duplicate database names could be added here

    let dbs = list_databases(src.clone()).await?;

    if dbs.contains(&request_body.name) {
        anyhow::bail!(
            "Database with name '{}' already exists in source '{}'",
            request_body.name,
            src
        );
    }

    let response = reqwest::Client::new()
        .post(config.url + "/v1/blaze/databases/create")
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

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let request_body = CreateSourceRequest {
        source_name: name.clone(),
    };

    let response = reqwest::Client::new()
        .post(config.url + "/v1/blaze/sources/create")
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
