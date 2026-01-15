use crate::core::HNSW;
use crate::server::service::database::search_database;
use crate::server::{EmbedRequest, EmbedResponse};
use crate::utils::{EmbeddingStore, Provider};
use crate::{error, info, warn};
use anyhow::{Context, Result};

/// Prefix for HNSW index files (batch-wise), for example: "hnsw_index_1", "hnsw_index_2", etc.
const INDEX_FILE_NAME: &str = "hnsw_index"; // TODO: Need to find other way to manage multiple indexes

pub async fn embed_run(request: EmbedRequest, _hnsw: Option<HNSW>) -> Result<EmbedResponse> {
    let batch_content = request.file_content; // TODO: Maybe change it to Vec<String>?
    let database_name = request.database.clone();
    let total_items: usize = batch_content.iter().map(|batch| batch.len()).sum();

    // Locate the database directory
    let database_path = match search_database(database_name.clone()).await {
        Ok(path) => path,
        Err(e) => {
            error!("Database '{}' not found", database_name);
            return Err(e).with_context(|| format!("Database '{}' not found", database_name));
        }
    };

    // Load latest HNSW from database directory if it exists, otherwise create a new one
    let (loaded_hnsw, max_index) = load_embeddings_index_from_database(database_name.clone()).await;
    let mut hnsw = match loaded_hnsw {
        Some(store) => store.hnsw_store,
        None => HNSW::new(18, 200, 12, 0.8),
    };

    // Load existing HNSW index if provided
    // if let Some(hnsw_index) = hnsw {
    //     info!(
    //         "Using provided HNSW index with {} nodes",
    //         hnsw_index.nodes.len()
    //     );
    //     // You can integrate the HNSW index with your embedding process here
    // } else {
    //     warn!("No HNSW index provided, proceeding without it");
    // }

    // TODO: Provider should not be set on the fly, Configure embedding provider
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::init(url, model);

    for (index, chunks) in batch_content.iter().enumerate() {
        let batch_index = index;

        // Fetch embeddings for the current chunk, and update HNSW index
        match provider.fetch_embeddings(chunks).await {
            Ok(embeddings) => {
                let embedded_count = embeddings.embedding.len();

                // Insert embeddings into HNSW index
                for (i, vector) in embeddings.embedding.iter().enumerate() {
                    let metadata = chunks.get(i).cloned().unwrap_or("[EMPTY]".to_string());
                    let random_level = hnsw.get_random_level();
                    hnsw.insert(vector.clone(), metadata, random_level);
                }

                let mut embedding_store = EmbeddingStore::new(hnsw.clone());

                let filename = database_path.join(format!(
                    "{}_{}",
                    INDEX_FILE_NAME,
                    batch_index + max_index + 1
                ));

                embedding_store
                    .write_to_disk(&filename)
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

    info!("Embedding complete: {} lines processed", total_items);

    Ok(EmbedResponse {
        database: database_name,
        total_entries: total_items,
    })
}

/// Load the lastest HNSW Index from the specified database
/// Returns the EmbeddingStore and the max index number found or (None, 0) if not found
pub async fn load_embeddings_index_from_database(
    database: String,
) -> (Option<EmbeddingStore>, usize) {
    info!("Reading embeddings from database '{}'", database);
    let database_name = match search_database(database.clone()).await {
        Ok(path) => path,
        Err(e) => {
            error!("Database '{}' not found, e: {}", database, e.to_string());
            return (None, 0);
        }
    };
    info!("Loading binary embeddings from: {:?}", database_name);

    let (loaded_hnsw, max_index) =
        match EmbeddingStore::load_lastest_index(INDEX_FILE_NAME, database_name.to_str().unwrap())
            .await
        {
            Ok((store, max_idx)) => (store, max_idx),
            Err(e) => {
                error!(
                    "Error loading embeddings from database '{}': {}",
                    database, e
                );
                (None, 0)
            }
        };

    if loaded_hnsw.is_none() {
        warn!(
            "No existing embeddings found in database, Creating one... '{}'",
            database
        );
    }

    (loaded_hnsw, max_index)
}
