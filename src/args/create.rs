use crate::core::ClientConfig;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use anyhow::Result;

pub async fn create_run(name: String, dimensions: usize) -> Result<()> {
    println!("Creating a new database...");

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let request_body = CreateDatabaseRequest { name, dimensions };

    let response = reqwest::Client::new()
        .post(config.url + "/create")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_json: CreateDatabaseResponse = response.json().await?;
        println!("Database created successfully with ID: {}", resp_json.id);
    } else {
        println!("Failed to create database. Status: {}", response.status());
    }

    Ok(())
}
