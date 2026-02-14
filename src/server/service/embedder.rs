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

//TODO: SERIOUSLY REFACTOR TOMORROW Both insert_run and embed_run have a lot of duplicated code, need to refactor later

/// Insert pre-computed embeddings into the specified database
pub async fn insert_run(
    request: &InsertRequest,
    _hnsw: Option<HNSW>,
    _provider: &Provider,
) -> Result<InsertResponse> {
    let vector_batch_data = &request.nodes;
    let database_name = &request.database;
    let source = &request.source;

    let total_entries = vector_batch_data
        .iter()
        .map(|batch| batch.len())
        .sum::<usize>();

    info!("Starting indexing with total entries: {}", total_entries);

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
    let inconsistent_vectors: Vec<Vec<&VectorDataDto>> = vector_batch_data
        .par_iter()
        .map(|vec_data| {
            vec_data
                .iter()
                .filter(|v| v.embedding.len() != expected_dimensions)
                .collect::<Vec<&VectorDataDto>>()
        })
        .filter(|inconsistent| !inconsistent.is_empty())
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
    let loaded_hnsw = load_index_from_database(database_name.clone(), source.clone()).await;
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
    // This is INTENTIONAL to prevent index corruption - DO NOT CHANGE, unless we implement a more sophisticated locking mechanism that allows concurrent writes with proper merging or versioning
    let _write_guard = lock.write().await;
    info!("Acquired write lock for database '{}'", database_name);

    let mut total_embedded = 0;
    for (index, vector_data) in vector_batch_data.iter().enumerate() {
        let batch_index = index;

        let embedded_count = vector_data.len();

        for (_index, vec_data) in vector_data.iter().enumerate() {
            let vector = &vec_data.embedding;
            let metadata = &vec_data.metadata;
            let random_level = hnsw.get_random_level();
            hnsw.insert(&vector, metadata.clone(), random_level);
        }
        total_embedded += embedded_count;
        info!(
            "Batch: {}, Embedded: {} Vectors (Total so far: {})",
            batch_index, embedded_count, total_embedded
        );
    }

    // Copy-on-Write (CoW) Replication Strategy:
    // Write to HNSW_INDEX_TEMP.bin
    // Atomic rename: TEMP → HNSW_INDEX.bin (crash-safe!)
    // Copy .bin → HNSW_INDEX.replica (exact snapshot for backups!)
    //
    // Benefits:
    // - .replica is EXACT copy of .bin (no version drift)
    // - Can backup .replica with ZERO locks (it's frozen)
    // - Simpler than rotation (no threshold logic needed)
    //
    // Cons: (TODO)
    // - Double disk I/O during writes (write temp, then copy to replica) - but this is acceptable for data safety and simplicity
    // - Should be performed asynchronously to avoid blocking the main thread, especially for large indexes or use background jobs

    let temp_filename = database_path.join("HNSW_INDEX_TEMP.bin");
    let current_filename = database_path.join("HNSW_INDEX.bin");
    let replica_filename = database_path.join("HNSW_INDEX.replica");

    let node_count = hnsw.nodes.len();
    let mut embedding_store = EmbeddingStore::new(hnsw);

    // Write to temporary file
    embedding_store
        .write_to_disk(&database_path.join("HNSW_INDEX_TEMP.bin"))
        .await
        .with_context(|| "Failed to write index to temp file")?;

    // Atomic rename (crash-safe! If crash here, previous .bin is still valid)
    tokio::fs::rename(&temp_filename, &current_filename)
        .await
        .with_context(|| "Failed to rename temp index to current")?;

    info!("Index saved: {} nodes total → HNSW_INDEX.bin", node_count);

    // Create replica snapshot (exact copy for backups) under write lock to ensure it's an exact snapshot of the current .bin
    // This is NON-CRITICAL, BUT FOR BACKUPS - if it fails, .bin is still valid
    if let Err(e) = tokio::fs::copy(&current_filename, &replica_filename).await {
        warn!("Failed to create replica snapshot (non-critical): {}", e);
    } else {
        info!("Created replica snapshot → HNSW_INDEX.replica");
    }

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

    info!("Starting embed_run: total_items={}", total_items);

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
    let loaded_hnsw = load_index_from_database(database_name.clone(), source.clone()).await;
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

    // Copy-on-Write (CoW) Replication Strategy (same as insert_run):
    //  Write to HNSW_INDEX_TEMP.bin
    //  Atomic rename: TEMP → HNSW_INDEX.bin (crash-safe!)
    //  Copy .bin → HNSW_INDEX.replica (exact snapshot for backups!)

    let temp_filename = database_path.join("HNSW_INDEX_TEMP.bin");
    let current_filename = database_path.join("HNSW_INDEX.bin");
    let replica_filename = database_path.join("HNSW_INDEX.replica");

    let node_count = hnsw.nodes.len();
    let mut embedding_store = EmbeddingStore::new(hnsw);

    // Write to temporary file
    embedding_store
        .write_to_disk(&database_path.join("HNSW_INDEX_TEMP.bin"))
        .await
        .with_context(|| "Failed to write index to temp file")?;

    // Atomic rename (crash-safe!)
    tokio::fs::rename(&temp_filename, &current_filename)
        .await
        .with_context(|| "Failed to rename temp index to current")?;

    info!("Index saved: {} nodes total → HNSW_INDEX.bin", node_count);

    // Create replica snapshot (exact copy for backups) under write lock to ensure it's an exact snapshot of the current .bin
    // This is NON-CRITICAL - if it fails, .bin is still valid
    if let Err(e) = tokio::fs::copy(&current_filename, &replica_filename).await {
        warn!("Failed to create replica snapshot (non-critical): {}", e);
    } else {
        info!("Created replica snapshot → HNSW_INDEX.replica");
    }

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

/// Loads the HNSW index from the database directory, with crash recovery fallback
/// Uses read lock to allow concurrent reads while blocking writes
pub async fn load_index_from_database(database: String, source: String) -> Option<EmbeddingStore> {
    info!("Reading embeddings from database '{}'", database);
    let database_name = match search_database_on_disk(&database, &source).await {
        Ok(path) => path,
        Err(e) => {
            error!("Database '{}' not found, e: {}", database, e.to_string());
            return None;
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

    // Try loading HNSW_INDEX.bin (current index)
    // If that fails, try HNSW_INDEX.replica (crash recovery fallback!)
    let bin_path = database_name.join("HNSW_INDEX.bin");
    let replica_path = database_name.join("HNSW_INDEX.replica");

    let loaded_hnsw = if bin_path.exists() {
        // Normal case: Load current index
        match EmbeddingStore::load_index_file(&bin_path).await {
            Ok(store) => {
                info!("Loaded current index: HNSW_INDEX.bin");
                Some(store)
            }
            Err(e) => {
                error!(
                    "Error loading HNSW_INDEX.bin from database '{}': {}",
                    database, e
                );
                None
            }
        }
    } else if replica_path.exists() {
        // Crash recovery: .bin missing/corrupted, but .replica exists
        // This happens if server crashed during write or .bin got corrupted
        warn!(
            "[CRASH RECOVERY] HNSW_INDEX.bin not found/corrupted for '{}', recovering from HNSW_INDEX.replica",
            database
        );
        match EmbeddingStore::load_index_file(&replica_path).await {
            Ok(mut store) => {
                info!("Successfully recovered from HNSW_INDEX.replica");

                // Drop read lock BEFORE writing to disk (can't write while holding read lock!)
                drop(_read_guard);
                info!("Released read lock for database '{}'", database);

                // CRITICAL: Write recovered index back to .bin to restore disk consistency
                warn!("[CRASH RECOVERY] Acquiring write lock to restore HNSW_INDEX.bin");
                let _write_guard = lock.write().await;
                info!("[CRASH RECOVERY] Acquired write lock for recovery");

                // Check again if .bin still doesn't exist (race condition check)
                // Another thread might have recovered while we were waiting for lock
                if bin_path.exists() {
                    warn!(
                        "[CRASH RECOVERY] HNSW_INDEX.bin already exists (recovered by another thread)"
                    );
                    drop(_write_guard);
                    return Some(store);
                }

                warn!("[CRASH RECOVERY] Writing recovered index back to HNSW_INDEX.bin");
                if let Err(e) = store.write_to_disk(&bin_path).await {
                    error!(
                        "[CRASH RECOVERY] Failed to write recovered index to disk: {}. \
                        Index loaded in memory but disk state inconsistent!",
                        e
                    );
                    // Still return the store - it's usable in memory even if disk write failed
                } else {
                    info!("[CRASH RECOVERY] Successfully restored HNSW_INDEX.bin from replica");
                }

                drop(_write_guard);
                info!("[CRASH RECOVERY] Released write lock");

                return Some(store);
            }
            Err(e) => {
                error!(
                    "Error loading HNSW_INDEX.replica from database '{}': {}",
                    database, e
                );
                None
            }
        }
    }
    // TODO: We could to restore from backups
    else {
        // No index at all - first time or corrupted
        None
    };

    if loaded_hnsw.is_none() {
        warn!(
            "No existing embeddings found in database, Creating new index... '{}'",
            database
        );
    }

    drop(_read_guard);
    info!("Released read lock for database '{}'", database);

    // Return (store) - we don't use version numbers anymore
    loaded_hnsw
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
