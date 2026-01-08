use crate::core::ClientConfig;
use crate::prelude::Ingestor;
use crate::server::{EmbedRequest, EmbedResponse};
use anyhow::Result;
use std::path::PathBuf;

pub async fn embed_run(file_path: PathBuf, database: String, batch: Option<usize>) -> Result<()> {
    println!("Embedding data into database...: {}", &database);

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let batch = batch.unwrap_or(512);

    let ingest = Ingestor::new(&file_path, batch);

    // Use smart chunking instead of line-by-line for better semantic search
    let content = ingest.read_chunks(100, 50)?;

    println!(
        "Created {} batches of semantic chunks for embedding",
        content.len()
    );

    let request_body = EmbedRequest {
        file_content: content,
        database,
        batch,
    };

    let response = reqwest::Client::new()
        .post(config.url + "/embed")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_json: EmbedResponse = response.json().await?;
        println!(
            "Data embedded successfully. Totals embeddings: {}",
            resp_json.total_lines
        );
    } else {
        println!("Failed to embed data. Status: {}", response.status());
    }

    Ok(())
}
