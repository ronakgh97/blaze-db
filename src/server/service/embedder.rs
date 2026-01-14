use crate::core::HNSW;
use crate::server::service::database::search_database;
use crate::server::{EmbedRequest, EmbedResponse};
use crate::utils::{EmbeddingStore, Provider};
use crate::{error, info, warn};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Prefix for HNSW index files (batch-wise), for example: "hnsw_index_1", "hnsw_index_2", etc.
const INDEX_FILE_NAME: &str = "hnsw_index_"; // TODO: Need to find other way to manage multiple indexes

pub async fn embed_run(request: EmbedRequest, _hnsw: Option<HNSW>) -> Result<EmbedResponse> {
    let batch_content = request.file_content; // TODO: Maybe change it to Vec<String>?
    let database_name = request.database.clone();
    let total_items: usize = batch_content.iter().map(|batch| batch.len()).sum();

    // Locate the database directory
    let database_path = match search_database(database_name.clone()).await {
        Ok(path) => path,
        Err(_e) => {
            error!("Database '{}' not found", database_name);
            return Err(_e).with_context(|| format!("Database '{}' not found", database_name));
        }
    };

    // Load latest HNSW from database directory if it exists, otherwise create a new one
    let (loaded_hnsw, max_index) =
        load_embeddings_index_from_database(database_name.clone()).await?;
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

    for (index, chunk) in batch_content.iter().enumerate() {
        let batch_index = index;

        // Fetch embeddings for the current chunk, and update HNSW index
        match provider.fetch_embeddings(chunk).await {
            Ok(embeddings) => {
                let embedded_count = embeddings.embedding.len();

                // Insert embeddings into HNSW index
                for (_i, vector) in embeddings.embedding.iter().enumerate() {
                    let random_level = hnsw.get_random_level();
                    hnsw.insert(vector.clone(), random_level);
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
        total_lines: total_items,
    })
}

/// Load the lastest HNSW Index from the specified database
/// Returns the EmbeddingStore and the max index number found
pub async fn load_embeddings_index_from_database(
    database: String,
) -> Result<(Option<EmbeddingStore>, usize)> {
    info!("Reading embeddings from database '{}'", database);
    let database_name = match search_database(database.clone()).await {
        Ok(path) => path,
        Err(_e) => {
            error!("Database '{}' not found", database);
            return Err(_e).with_context(|| format!("Database '{}' not found", database));
        }
    };
    info!("Loading binary embeddings from: {:?}", database_name);

    let (loaded_hnsw, max_index) = {
        let mut latest_path: Option<PathBuf> = None;
        let mut max_num = 0;
        for entry in std::fs::read_dir(&database_name)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(suffix) = file_name.strip_prefix(INDEX_FILE_NAME) {
                    if let Ok(num) = suffix.parse::<usize>() {
                        if num > max_num {
                            max_num = num;
                            latest_path = Some(path);
                        }
                    }
                }
            }
        }
        let loaded = if let Some(path) = latest_path {
            info!(
                "Loaded latest HNSW index: {:?} from database: {:?}",
                path.file_name(),
                database_name
            );
            Some(EmbeddingStore::load_binary_file(&path).await?)
        } else {
            warn!("No HNSW index files found in database, creating new one...");
            None
        };
        (loaded, max_num)
    };

    Ok((loaded_hnsw, max_index))
}
