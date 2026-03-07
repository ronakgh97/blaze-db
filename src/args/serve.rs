use crate::core::{check_source_valid, list_sources};
use crate::server::start_server;
use crate::utils::Provider;
use crate::{error, info, warn};
use anyhow::Result;
use dotenv::dotenv;

pub async fn serve_run(
    cli_port: Option<u16>,
    enable_backup: bool,
    disable_env: bool,
    sandbox: bool,
    _source: Option<Vec<String>>,
) -> Result<Provider> {
    info!("Starting the Server...");

    if sandbox {
        info!("Running in sandbox mode with in-memory storage (no persistence)");
        // TODO: Implement sandbox mode with in-memory storage
    }

    // Get all sources from ServerFile
    let all_sources = list_sources().await?;

    if all_sources.is_empty() {
        warn!("No sources found in server data, using default source: default_src");
    }

    let sources_to_check = if all_sources.is_empty() {
        vec!["default_src".to_string()]
    } else {
        all_sources
    };

    let provider = if disable_env {
        info!("Running in no-env mode");
        Provider::init_mock(1024)
    } else {
        dotenv().ok();

        let url = std::env::var("EMBEDDING_API_URL")
            .expect("EMBEDDING_API_URL environment variable is required");
        let model = std::env::var("EMBEDDING_MODEL")
            .expect("EMBEDDING_MODEL environment variable is required");
        let api_key = std::env::var("EMBEDDING_API_KEY")
            .expect("EMBEDDING_API_KEY environment variable is required");

        Provider::init(url, model, api_key)
    };
    info!("{:?}", Provider::pretty_display(&provider));

    let final_port = if let Some(p) = cli_port {
        p
    } else if let Ok(env_port) = std::env::var("PORT") {
        env_port
            .parse::<u16>()
            .expect("PORT must be a valid number")
    } else {
        8080
    };

    // Validate all sources before starting server
    let mut valid_sources = Vec::new();
    for src in &sources_to_check {
        if !check_source_valid(src).await? {
            warn!("Source: {} is not valid, skipping", src);
        } else {
            valid_sources.push(src.clone());
        }
    }

    if valid_sources.is_empty() {
        error!("No valid sources found in {:?}", sources_to_check);
        return Err(anyhow::anyhow!("No valid sources found"));
    }

    info!(
        "Starting server with {} valid source(s)",
        valid_sources.len()
    );
    start_server(final_port, valid_sources, enable_backup, &provider).await?;

    Ok(provider)
}
