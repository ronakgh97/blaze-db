use crate::core::check_source_valid;
use crate::prelude::ServerConfig;
use crate::server::start_server;
use crate::utils::Provider;
use crate::{error, info, warn};
use anyhow::Result;

pub async fn serve_run(port: Option<u16>, _source: Option<Vec<String>>) -> Result<Provider> {
    info!("Starting the Server...");

    let config =
        ServerConfig::load_config(&ServerConfig::get_default_server_config_path()?).await?;

    let source = config.data_source.source_name.clone().unwrap_or_else(|| {
        warn!("No source provided in config, using default source: default_src");
        vec!["default_src".to_string()]
    });

    // Check env for port override
    let env_port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());
    let port = env_port.or(port);

    // Use 8080, if no one cares about port :)
    let port = port.unwrap_or(8080);

    let url = std::env::var("EMBEDDING_API_URL")
        .unwrap_or_else(|_| "http://localhost:1234/v1/embeddings".to_string());

    let model = std::env::var("EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-qwen3-embedding-0.6b".to_string());

    let api_key = std::env::var("EMBEDDING_API_KEY").unwrap_or_else(|_| "local".to_string());

    // Init provider at the start of the server
    let provider = Provider::init(url, model, api_key);
    info!("Using Embedding Provider: \n{:?}", provider);

    // // Get sources or use all sources from config
    // let source = if let Some(src) = source {
    //     src
    // } else {
    //     warn!("No source provided, using default source: default_src");
    //     config
    //         .data_source
    //         .source_name
    //         .unwrap_or_else(|| vec!["default_src".to_string()])
    // };

    // Validate all sources before starting server
    let mut valid_sources = Vec::new();
    for src in &source {
        if check_source_valid(&src).await? {
            info!("Source: {} is valid", src);
            valid_sources.push(src.clone());
        } else {
            warn!("Source: {} is not valid, skipping", src);
        }
    }

    if valid_sources.is_empty() {
        error!("No valid sources found in {:?}", source);
        return Err(anyhow::anyhow!("No valid sources found"));
    }

    info!(
        "Starting server with {} valid source(s)",
        valid_sources.len()
    );
    start_server(port, valid_sources, &provider).await?;

    Ok(provider)
}
