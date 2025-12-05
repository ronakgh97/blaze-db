use crate::prelude::Ingestor;
use crate::server::{EmbedRequest, EmbedResponse};
use anyhow::Result;
use std::path::PathBuf;

pub async fn embed_run(file_path: PathBuf, database: String, batch: Option<usize>) -> Result<()> {
    println!("Embedding data into database...: {}", &database);

    let batch = batch.unwrap_or(512);

    let ingest = Ingestor::new(&file_path, batch);
    let content = ingest.read_line()?;

    let request_body = EmbedRequest {
        file_content: content,
        database,
        batch,
    };

    let response = reqwest::Client::new()
        .post("http://127.0.0.1:8001/embed")
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
