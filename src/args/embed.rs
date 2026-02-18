use crate::core::UserConfig;
use crate::prelude::Ingestor;
use crate::server::{EmbedData, EmbedRequest, EmbedResponse};
use anyhow::Result;
use std::path::PathBuf;
use uuid::Uuid;

pub async fn embed_run(
    file_path: PathBuf,
    database: String,
    src: String,
    batch: Option<usize>,
) -> Result<()> {
    println!("Embedding data into database...: {}", &database);

    let config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    let batch = batch.unwrap_or(1024);

    let ingest = Ingestor::new(&file_path, batch);

    let content = ingest.read_chunks(150, 50)?;

    let content: Vec<Vec<EmbedData>> = content
        .into_iter()
        .map(|batch| {
            batch
                .into_iter()
                .map(|item| EmbedData {
                    id: Uuid::new_v4().to_string(),
                    embed_data: item,
                })
                .collect()
        })
        .collect();

    let total_chunks: usize = content.iter().map(|b| b.len()).sum();

    println!(
        "Created {} batches with {} total chunks for embedding",
        content.len(),
        total_chunks
    );

    let request_body = EmbedRequest {
        batch_content: content,
        database,
        source: src,
        batch,
    };

    dotenv::dotenv().ok();
    let api_key = std::env::var("BLAZE_API_KEY").unwrap_or("local_dev_key".to_string());

    let response = reqwest::Client::new()
        .post(config.server.instance_url + "/v1/blazedb/embed")
        .header("Authorization", format!("Bearer {}", api_key))
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
