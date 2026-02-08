use crate::core::{HNSW, SERVER_FILE, check_source_valid};
use crate::server::controller::{DB_WRITE_LOCKS, ErrorTypes};
use crate::server::dto::VectorDataDto;
use crate::server::service::database::search_database_on_disk;
use crate::server::{EmbedRequest, EmbedResponse, InsertRequest, InsertResponse};
use crate::utils::{EmbeddingStore, Provider};
use crate::{error, info, warn};
use anyhow::{Context, Result};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Prefix for HNSW index files (batch-wise), for example: "hnsw_index_1", "hnsw_index_2", etc.
pub const INDEX_FILE_NAME: &str = "HNSW_INDEX"; // TODO: Need to find other way to manage multiple indexes

//TODO: Both insert_run and embed_run have a lot of duplicated code, need to refactor later

/// Insert pre-computed embeddings into the specified database
pub async fn insert_run(
    request: &InsertRequest,
    _hnsw: Option<HNSW>,
    _provider: &Provider,
) -> Result<InsertResponse> {
    let vector_data = &request.vectors;
    let database_name = &request.database;
    let source = &request.source;

    let total_entries = vector_data.len();

    // Checks source and database existence
    if !check_source_valid(&source).await? {
        error!("Source '{}' not found", source);
        return Err(ErrorTypes::SourceNotFound(format!("Source '{}' not found", source)).into());
    }

    // Check all vector dimensions consistency with database init (dimensions)
    let (expected_dimensions, metrics) = {
        let server_file = SERVER_FILE.read().await;
        match server_file.get_vector_base(&source, &database_name)? {
            Some(vb) => (vb.dimension as usize, vb.metric_type),
            None => {
                error!(
                    "Database '{}' not found in source '{}'",
                    database_name, source
                );
                return Err(ErrorTypes::DatabaseNotFound(format!(
                    "Database '{}' not found in source '{}'",
                    database_name, source
                ))
                .into());
            }
        }
    };

    // DO THIS LAST TO AVOID WASTING TIME
    // VERY IMPORTANT!!, OR ELSE IT WILL CORRUPT THE INDEX WITH INCONSISTENT DIMENSIONS
    // Fast check in parallel using rayon
    let inconsistent_vectors: Vec<&VectorDataDto> = vector_data
        .par_iter()
        .filter(|vec_data| vec_data.embedding.len() != expected_dimensions)
        .collect();

    if !inconsistent_vectors.is_empty() {
        error!(
            "Found {} vectors with inconsistent dimensions",
            inconsistent_vectors.len(),
        );
        return Err(ErrorTypes::InvalidField(format!(
            "Inconsistent vector dimensions found. Expected: {}, but some vectors have different dimensions.",
            expected_dimensions
        ))
        .into());
    }

    // Locate the database directory
    let database_path = search_database_on_disk(&database_name, &source)
        .await
        .map_err(|e| {
            error!(
                "Database '{}' not found in source '{}'",
                database_name, source
            );
            ErrorTypes::DatabaseNotFound(format!(
                "Database '{}' not found in source '{}': {}",
                database_name, source, e
            ))
        })?;

    // Load latest HNSW from database directory if it exists, otherwise create a new one
    let (loaded_hnsw, max_index) =
        load_embeddings_index_from_database(database_name.clone(), source.clone()).await;
    let mut hnsw = match loaded_hnsw {
        Some(store) => store.hnsw_store,
        None => HNSW::new(18, 200, 12, 0.8, &Some(metrics)),
    };

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
    // TODO: PERFORMANCE - Write lock held during HNSW insertions + disk I/O
    // Lock duration: ~100-500ms depending on batch size and disk speed
    // This is INTENTIONAL to prevent index corruption - DO NOT CHANGE
    // Alternative optimization: Use copy-on-write or shadow index technique
    let _write_guard = lock.write().await;
    info!("Acquired write lock for database '{}'", database_name);

    // TODO: Use batch-wise
    // let mut total_embedded = 0;
    for (_index, vector_data) in vector_data.iter().enumerate() {
        // let batch_index = index;

        // This is just inserting pre-computed embeddings, so no need to fetch from provider
        let embeddings = &vector_data.embedding;
        let metadata = &vector_data.metadata;

        // Insert embeddings into HNSW index
        let random_level = hnsw.get_random_level();
        hnsw.insert(embeddings, metadata.clone(), random_level);
    }

    // TODO: Is there a better method to manage multiple index files? or merge them? or overwrite them lastest ones? or prune last 'N' indexes?
    // Save the final cumulative index ONCE at the end
    let final_index_number = max_index + 1;
    let final_filename = database_path.join(format!("{}_{}", INDEX_FILE_NAME, final_index_number));

    let node_count = hnsw.nodes.len();
    let mut embedding_store = EmbeddingStore::new(hnsw);
    embedding_store
        .write_to_disk(&final_filename, final_index_number)
        .await
        .with_context(|| "Failed to write final index")?;

    info!(
        "Final index saved: {} nodes total → {:?}",
        node_count,
        final_filename.display()
    );

    // Write lock will be automatically released here when _write_guard goes out of scope
    drop(_write_guard);
    info!("Released write lock for database '{}'", database_name);

    // Update node_count in SERVER_FILE
    // TODO: This acquires SERVER_FILE write lock after releasing per-db lock
    // Could be optimized, but keeping it simple for now
    {
        let mut server_file = SERVER_FILE.write().await;
        if let Err(e) = server_file.update_node_count(&source, &database_name, node_count as u32) {
            warn!(
                "Failed to update node_count for database '{}': {}",
                database_name, e
            );
            // Don't fail the operation - metadata update is not critical
        } else {
            info!(
                "Updated metadata: node_count={} for database '{}'",
                node_count, database_name
            );
        }
    }

    Ok(InsertResponse {
        database: database_name.clone(),
        source: source.clone(),
        total_inserted: total_entries,
    })
}

/// Embed the provided batch content into the specified database
pub async fn embed_run(
    request: EmbedRequest,
    _hnsw: Option<HNSW>,
    provider: &Provider,
) -> Result<EmbedResponse> {
    let batch_content = &request.batch_content; // TODO: Maybe change it to Vec<String>?
    let database_name = &request.database;
    let source = &request.source;

    let total_items: usize = batch_content.iter().map(|batch| batch.len()).sum();

    // Checks source and database existence
    if !check_source_valid(&source).await? {
        error!("Source '{}' not found", source);
        return Err(ErrorTypes::SourceNotFound(format!("Source '{}' not found", source)).into());
    }

    let metrics = {
        let server_file = SERVER_FILE.read().await;
        match server_file.get_vector_base(&source, &database_name)? {
            Some(vb) => vb.metric_type,
            None => {
                error!(
                    "Database '{}' not found in source '{}'",
                    database_name, source
                );
                return Err(ErrorTypes::DatabaseNotFound(format!(
                    "Database '{}' not found in source '{}'",
                    database_name, source
                ))
                .into());
            }
        }
    };

    // Locate the database directory
    let database_path = search_database_on_disk(&database_name, &source)
        .await
        .map_err(|e| {
            error!(
                "Database '{}' not found in source '{}'",
                database_name, source
            );
            ErrorTypes::DatabaseNotFound(format!(
                "Database '{}' not found in source '{}': {}",
                database_name, source, e
            ))
        })?;

    // Load latest HNSW from database directory if it exists, otherwise create a new one
    let (loaded_hnsw, max_index) =
        load_embeddings_index_from_database(database_name.clone(), source.clone()).await;
    let mut hnsw = match loaded_hnsw {
        Some(store) => store.hnsw_store,
        None => HNSW::new(18, 200, 12, 0.8, &Some(metrics)),
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
    // TODO: PERFORMANCE - Write lock held during embedding generation + HNSW insertions + disk I/O
    // Lock duration: Can be VERY long (seconds) for large batches
    // This prevents concurrent writes to same DB - necessary for index consistency
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
                    hnsw.insert(&vector, metadata, random_level);
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

    let node_count = hnsw.nodes.len();
    let mut embedding_store = EmbeddingStore::new(hnsw);
    embedding_store
        .write_to_disk(&final_filename, final_index_number)
        .await
        .with_context(|| "Failed to write final index")?;

    info!(
        "Final index saved: {} nodes total → {:?}",
        node_count,
        final_filename.display()
    );

    // Write lock will be automatically released here when _write_guard goes out of scope
    drop(_write_guard);
    info!("Released write lock for database '{}'", database_name);

    // Update node_count in SERVER_FILE
    {
        let mut server_file = SERVER_FILE.write().await;
        if let Err(e) = server_file.update_node_count(&source, &database_name, node_count as u32) {
            warn!(
                "Failed to update node_count for database '{}': {}",
                database_name, e
            );
            // Don't fail the operation - metadata update is not critical
        } else {
            info!(
                "Updated metadata: node_count={} for database '{}'",
                node_count, database_name
            );
        }
    }

    Ok(EmbedResponse {
        database: database_name.clone(),
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
    let database_name = match search_database_on_disk(&database, &source).await {
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

// TODO: Schedule this to run periodically in background task (use this)

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
