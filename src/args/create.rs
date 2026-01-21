use crate::core::ClientConfig;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use anyhow::Result;
use colored::Colorize;

pub async fn create_run(name: String, src: String, dimensions: usize) -> Result<()> {
    println!("Creating a new database: {}\n", name.yellow());

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let request_body = CreateDatabaseRequest {
        name,
        source: src,
        dimensions,
    };

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
    } else {
        println!("Failed to create database. Status: {}", response.status());
    }

    Ok(())
}
