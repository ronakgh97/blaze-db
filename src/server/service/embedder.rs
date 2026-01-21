use crate::core::HNSW;
use crate::server::controller::DB_WRITE_LOCKS;
use crate::server::service::database::search_database;
use crate::server::{EmbedRequest, EmbedResponse, InsertRequest};
use crate::utils::{EmbeddingStore, Provider};
use crate::{error, info, warn};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Prefix for HNSW index files (batch-wise), for example: "hnsw_index_1", "hnsw_index_2", etc.
pub const INDEX_FILE_NAME: &str = "HNSW_INDEX"; // TODO: Need to find other way to manage multiple indexes

#[allow(unused)]
pub async fn insert_run(request: InsertRequest, _hnsw: Option<HNSW>) -> Result<EmbedResponse> {
    unimplemented!("Direct insert_run is not implemented yet");
}

pub async fn embed_run(request: EmbedRequest, _hnsw: Option<HNSW>) -> Result<EmbedResponse> {
    let batch_content = request.batch_content; // TODO: Maybe change it to Vec<String>?
    let database_name = request.database.clone();
    let source = request.source.clone();

    let total_items: usize = batch_content.iter().map(|batch| batch.len()).sum();

    // Locate the database directory
    let database_path = match search_database(database_name.clone(), source.clone()).await {
        Ok(path) => path,
        Err(e) => {
            error!("Database '{}' not found", database_name);
            return Err(e).with_context(|| format!("Database '{}' not found", database_name));
        }
    };

    // Load latest HNSW from database directory if it exists, otherwise create a new one
    let (loaded_hnsw, max_index) =
        load_embeddings_index_from_database(database_name.clone(), source.clone()).await;
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
    // } else {
    //     warn!("No HNSW index provided, proceeding without it");
    // }

    // Configure embedding provider from env or use defaults
    let url = std::env::var("EMBEDDING_API_URL")
        .unwrap_or_else(|_| "http://localhost:1234/v1/embeddings".to_string());
    let model = std::env::var("EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-qwen3-embedding-0.6b".to_string());
    let provider = Provider::init(url, model);

    // Get or create lock for this database
    let lock = {
        let mut locks = DB_WRITE_LOCKS.lock().await;
        locks
            .entry(database_name.clone())
            .or_insert_with(|| Arc::new(RwLock::new(())))
            .clone()
    };

    // Acquire write lock - this ensures only one write operation per database at a time
    // Multiple databases can still be written to concurrently
    let _write_guard = lock.write().await;
    info!("Acquired write lock for database '{}'", database_name);

    let mut total_embedded = 0;
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

                total_embedded += embedded_count;
                info!(
                    "Batch: {}, Embedded: {} Vectors (Total so far: {})",
                    batch_index, embedded_count, total_embedded
                );
            }
            Err(e) => {
                error!("Error fetching embeddings for batch: {}", batch_index);
                return Err(e).with_context(|| format!("Failed to embed batch {}", batch_index));
            }
        }
    }

    // TODO: Is there a better method to manage multiple index files? or merge them? or overwrite them lastest ones? or prune last 'N' indexes?
    // Save the final cumulative index ONCE at the end
    let final_index_number = max_index + 1;
    let final_filename = database_path.join(format!("{}_{}", INDEX_FILE_NAME, final_index_number));

    let mut embedding_store = EmbeddingStore::new(hnsw.clone());
    embedding_store
        .write_to_disk(&final_filename)
        .await
        .with_context(|| "Failed to write final index")?;

    info!(
        "Final index saved: {} nodes total → {:?}",
        hnsw.nodes.len(),
        final_filename.display()
    );

    // Write lock will be automatically released here when _write_guard goes out of scope
    drop(_write_guard);
    info!("Released write lock for database '{}'", database_name);

    Ok(EmbedResponse {
        database: database_name,
        source: source.clone(),
        total_entries: total_items,
    })
}

/// Load the lastest HNSW Index from the specified database
/// Returns the EmbeddingStore and the max index number found or (None, 0) if not found
/// Uses read lock to allow concurrent reads while blocking writes
pub async fn load_embeddings_index_from_database(
    database: String,
    source: String,
) -> (Option<EmbeddingStore>, usize) {
    info!("Reading embeddings from database '{}'", database);
    let database_name = match search_database(database.clone(), source).await {
        Ok(path) => path,
        Err(e) => {
            error!("Database '{}' not found, e: {}", database, e.to_string());
            return (None, 0);
        }
    };
    info!("Loading binary embeddings from: {:?}", database_name);

    // Acquire read lock to allow concurrent reads but block if a write is in progress
    let lock = {
        let mut locks = DB_WRITE_LOCKS.lock().await;
        locks
            .entry(database.clone())
            .or_insert_with(|| Arc::new(RwLock::new(())))
            .clone()
    };

    let _read_guard = lock.read().await;
    info!("Acquired read lock for database '{}'", database);

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

    drop(_read_guard);
    info!("Released read lock for database '{}'", database);

    (loaded_hnsw, max_index)
}

/// Cleanup old index files, keeping only the last N indexes
/// This prevents disk space from filling up with old indexes
#[allow(unused)]
async fn cleanup_old_indexes(db_path: &PathBuf, prefix: &str, keep_last: usize) -> Result<()> {
    let mut index_files: Vec<(usize, PathBuf)> = Vec::new();

    // Scan directory for index files
    for entry in std::fs::read_dir(db_path)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(suffix) = file_name.strip_prefix(prefix) {
                let suffix = suffix.strip_suffix(".bin").unwrap_or(suffix);
                let suffix = suffix.strip_prefix('_').unwrap_or(suffix);
                if let Ok(num) = suffix.parse::<usize>() {
                    index_files.push((num, path));
                }
            }
        }
    }

    // Sort by index number (ascending)
    index_files.sort_by_key(|(num, _)| *num);

    // Delete all except last N
    if index_files.len() > keep_last {
        let to_delete = index_files.len() - keep_last;
        for (idx, path) in index_files.iter().take(to_delete) {
            info!("Cleaning up old index: {} at {:?}", idx, path);
            tokio::fs::remove_file(path)
                .await
                .with_context(|| format!("Failed to delete old index: {:?}", path))?;
        }
        info!(
            "Cleanup complete: deleted {} old index(es), kept last {}",
            to_delete, keep_last
        );
    }

    Ok(())
}
