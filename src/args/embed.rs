use crate::core::ClientConfig;
use crate::prelude::Ingestor;
use crate::server::{EmbedRequest, EmbedResponse};
use anyhow::Result;
use std::path::PathBuf;

pub async fn embed_run(file_path: PathBuf, database: String, batch: Option<usize>) -> Result<()> {
    println!("Embedding data into database...: {}", &database);

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let batch = batch.unwrap_or(1024);

    let ingest = Ingestor::new(&file_path, batch);

    let content = ingest.read_chunks(150, 50)?;

    let total_chunks: usize = content.iter().map(|b| b.len()).sum();

    println!(
        "Created {} batches with {} total chunks for embedding",
        content.len(),
        total_chunks
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
            "Data embedded successfully. Total items: {}",
            resp_json.total_entries
        );
    } else {
        println!("Failed to embed data. Status: {}", response.status());
    }

    Ok(())
}
