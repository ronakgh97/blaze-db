use crate::core::UserConfig;
use crate::prelude::{ListResponse, Metrics};
use crate::server::{
    CreateDatabaseRequest, CreateDatabaseResponse, CreateSourceRequest, CreateSourceResponse,
};
use anyhow::Result;
use colored::Colorize;
use reqwest::Client;
use std::ops::Add;

// Create either a new source and database, or just a new database within an existing source, based on option args
pub async fn create_run(
    name: Option<String>,
    src: String,
    metrics: Option<Metrics>,
    dimensions: Option<usize>,
) -> Result<()> {
    let client = Client::new();
    let dim = dimensions.unwrap_or(1024);
    let config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    dotenv::dotenv().ok();
    let api_key = std::env::var("BLAZE_API_KEY").unwrap_or("local_dev_key".to_string());

    let list_request = Client::new()
        .get(config.server.instance_url.to_string() + "/v1/blazedb/list")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    let list_response: Vec<ListResponse> = list_request.json().await?;

    // Check if source already exists, then skip creation
    let source_exists = list_response
        .iter()
        .any(|source_data| source_data.from_sources == src);

    if let Some(name) = name {
        if !source_exists {
            create_source(&src, &client, &config, &api_key).await?;
        }
        println!(
            "Creating database ({}) in source ({})",
            name.yellow(),
            src.yellow()
        );
        create_database(name, metrics, &src, dim, &client, &config, &api_key).await?;

        return Ok(());
    }

    println!("Creating source ({})", src.yellow());
    create_source(&src, &client, &config, &api_key).await?;

    Ok(())
}

async fn create_database(
    name: String,
    metrics: Option<Metrics>,
    src: &String,
    dimensions: usize,
    client: &Client,
    config: &UserConfig,
    api_key: &String,
) -> Result<()> {
    let request_body = CreateDatabaseRequest {
        name,
        source: src.clone(),
        metrics,
        dimensions,
    };

    let response = client
        .post(
            &config
                .server
                .instance_url
                .to_string()
                .add("/v1/blazedb/databases/create"),
        )
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

async fn create_source(
    name: &String,
    client: &Client,
    config: &UserConfig,
    api_key: &String,
) -> Result<()> {
    let request_body = CreateSourceRequest {
        source_name: name.to_string(),
    };

    let response = client
        .post(
            &config
                .server
                .instance_url
                .to_string()
                .add("/v1/blazedb/sources/create"),
        )
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
