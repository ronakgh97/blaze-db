use crate::server::service::database::search_database;
use crate::server::{EmbedRequest, EmbedResponse};
use crate::utils::{EmbeddingStore, Provider, VectorData, log};
use crate::{error, info};
use anyhow::{Context, Result};

pub async fn embed_run(request: EmbedRequest) -> Result<EmbedResponse> {
    let batch_content = request.file_content;
    let database_name = request.database.clone();

    // Locate the database directory
    let database_path = match { search_database(database_name.clone()).await } {
        Ok(path) => path,
        Err(_e) => {
            error!("Database '{}' not found", database_name);
            return Err(_e).with_context(|| format!("Database '{}' not found", database_name));
        }
    };

    //TODO: Provider should not be set on the fly
    // Configure embedding provider
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::new(url, model);

    let total_lines: usize = batch_content.iter().map(|batch| batch.len()).sum();

    for (index, chunk) in batch_content.iter().enumerate() {
        let batch_index = index;

        match provider.fetch_embeddings(chunk).await {
            Ok(embeddings) => {
                let embedded_count = embeddings.data.len();
                let embedding_store = EmbeddingStore::new(batch_index, embeddings.data);

                let filename = database_path
                    .join(format!("embeddings_batch_{}", batch_index))
                    .to_string_lossy()
                    .to_string();

                embedding_store
                    .write_binary(&filename)
                    .await
                    .with_context(|| format!("Failed to write batch {}", batch_index))?;

                let path_display = std::path::Path::new(&filename);

                info!(
                    "Batch: {}, Embedded: {} Vectors >> {:?}",
                    batch_index,
                    embedded_count,
                    path_display.display()
                );
            }
            Err(e) => {
                error!("Error fetching embeddings for batch: {}", batch_index);
                return Err(e).with_context(|| format!("Failed to embed batch {}", batch_index));
            }
        }
    }

    info!("Embedding complete: {} lines processed", total_lines);

    Ok(EmbedResponse {
        database: database_name,
        total_lines,
    })
}

/// Read embeddings from the specified database
pub async fn read_embeddings_from_database(database: String) -> Result<VectorData> {
    let database_path = match { search_database(database.clone()).await } {
        Ok(path) => path,
        Err(_e) => {
            error!("Database '{}' not found", database);
            return Err(_e).with_context(|| format!("Database '{}' not found", database));
        }
    };

    let embedding_store = EmbeddingStore::read_binary(database_path.to_str().unwrap()).await?;

    Ok(embedding_store)
}
