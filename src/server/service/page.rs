use crate::server::controller::{ErrorTypes, INDEX_CACHE, LOADING_LOCKS};
use crate::server::dto::{GetIndexDetailsRequest, GetIndexDetailsResponse, VectorDataDto};
use crate::server::service::database::search_database_on_disk;
use crate::server::service::load_index_from_database;
use crate::utils::read_embeddings_metadata;
use crate::{debug, error, info, trace};
use anyhow::Result;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use std::sync::Arc;
use std::time::Instant;

/// Number of entries returned per page
pub const PAGE_SIZE: usize = 1024;

/// Pagination math helper.
/// Returns `(total_pages, clamped_page, start_idx, end_idx)`.
/// - `requested_page` is 1-based; 0 or out-of-bounds → page 1.
/// - `total_pages` is at least 1 even when there are no entries.
#[inline]
pub fn compute_page(total_vectors: usize, requested_page: usize) -> (usize, usize, usize, usize) {
    let total_pages = (total_vectors.max(1) + PAGE_SIZE - 1) / PAGE_SIZE;
    let page = if requested_page == 0 || requested_page > total_pages {
        1
    } else {
        requested_page
    };
    let start = (page - 1) * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(total_vectors);
    (total_pages, page, start, end)
}

pub async fn get_index_by_page(request: GetIndexDetailsRequest) -> Result<GetIndexDetailsResponse> {
    let db_name = &request.database;
    let source = &request.source;

    // Resolve database directory
    let db_path = search_database_on_disk(db_name, source)
        .await
        .map_err(|e| {
            error!("Database '{}' not found in source '{}'", db_name, source);
            anyhow::anyhow!(ErrorTypes::DatabaseNotFound(format!(
                "Database '{}' not found in source '{}': {}",
                db_name, source, e
            )))
        })?;

    let io_start = Instant::now();
    let cache_key = format!("{}_{}", db_name, source);

    {
        let cache = INDEX_CACHE.read().await;
        if let Some(cached) = cache.peek(&cache_key) {
            debug!("Cache HIT for database '{}'", db_name);

            let checksum_on_disk = match read_embeddings_metadata(&db_path).await {
                Ok(meta) => meta.checksum,
                Err(e) => {
                    error!("Failed to read metadata for cache validation: {}", e);
                    return Err(ErrorTypes::IndexNotFound(format!(
                        "Failed to validate cache: {}",
                        e
                    ))
                    .into());
                }
            };

            if cached.0.checksum == checksum_on_disk {
                debug!("Cache valid for '{}'", db_name);
                let io_sec = io_start.elapsed().as_secs_f64();
                let hnsw_index = cached.1.clone();
                let nodes = &hnsw_index.hnsw_store.nodes;

                let (total_pages, current_page, start, end) =
                    compute_page(nodes.len(), request.page);

                let entries = nodes[start..end]
                    .par_iter()
                    .filter(|n| !n.is_deleted()) // skip soft-deleted nodes
                    .map(|n| VectorDataDto {
                        id: n.node_id.clone(),
                        embedding: n.vector.clone(),
                        metadata: n.metadata.clone(),
                    })
                    .collect();

                info!(
                    "[page] Served page {}/{} for '{}' ({:.4}s IO)",
                    current_page, total_pages, db_name, io_sec
                );

                return Ok(GetIndexDetailsResponse {
                    source: source.to_string(),
                    database: db_name.to_string(),
                    total_pages,
                    current_page,
                    entries,
                });
            }

            debug!("Cache stale for '{}', reloading", db_name);
        }
    }

    debug!("Cache MISS for database '{}'", db_name);

    let loading_lock = {
        let mut locks = LOADING_LOCKS.write().await;
        locks
            .get_or_insert_mut(cache_key.clone(), || Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };

    let _load_guard = loading_lock.lock().await;
    trace!("Acquired loading lock for '{}'", db_name);

    // Double-check after acquiring loading lock
    {
        let cache = INDEX_CACHE.read().await;
        if let Some(cached) = cache.peek(&cache_key) {
            let checksum_on_disk = match read_embeddings_metadata(&db_path).await {
                Ok(meta) => meta.checksum,
                Err(e) => {
                    return Err(ErrorTypes::IndexNotFound(format!(
                        "Failed to validate cache: {}",
                        e
                    ))
                    .into());
                }
            };

            if cached.0.checksum == checksum_on_disk {
                debug!("Cache HIT after waiting on loading lock for '{}'", db_name);
                let io_sec = io_start.elapsed().as_secs_f64();
                let hnsw_index = cached.1.clone();
                let nodes = &hnsw_index.hnsw_store.nodes;

                let (total_pages, current_page, start, end) =
                    compute_page(nodes.len(), request.page);

                let entries = nodes[start..end]
                    .iter()
                    .filter(|n| !n.is_deleted())
                    .map(|n| VectorDataDto {
                        id: n.node_id.clone(),
                        embedding: n.vector.clone(),
                        metadata: n.metadata.clone(),
                    })
                    .collect();

                info!(
                    "[page] Served page {}/{} for '{}' ({:.4}s IO)",
                    current_page, total_pages, db_name, io_sec
                );

                return Ok(GetIndexDetailsResponse {
                    source: source.to_string(),
                    database: db_name.to_string(),
                    total_pages,
                    current_page,
                    entries,
                });
            }
        }
    }

    // Load from disk
    debug!("Loading index from disk for '{}'", db_name);
    let store = load_index_from_database(db_name.clone(), source.clone()).await;

    let metadata = match read_embeddings_metadata(&db_path).await {
        Ok(meta) => Arc::new(meta),
        Err(e) => {
            error!("Failed to read metadata: {}", e);
            return Err(
                ErrorTypes::IndexNotFound(format!("Failed to read metadata: {}", e)).into(),
            );
        }
    };

    let store = match store {
        Some(s) => Arc::new(s),
        None => {
            return Err(ErrorTypes::IndexNotFound("No index found in database".to_string()).into());
        }
    };

    {
        let mut cache = INDEX_CACHE.write().await;
        cache.put(cache_key, (Arc::clone(&metadata), Arc::clone(&store)));
    }

    trace!("Released loading lock for '{}'", db_name);

    let io_sec = io_start.elapsed().as_secs_f64();
    let nodes = &store.hnsw_store.nodes;
    let (total_pages, current_page, start, end) = compute_page(nodes.len(), request.page);

    let entries = nodes[start..end]
        .iter()
        .filter(|n| !n.is_deleted())
        .map(|n| VectorDataDto {
            id: n.node_id.clone(),
            embedding: n.vector.clone(),
            metadata: n.metadata.clone(),
        })
        .collect();

    info!(
        "[page] Served page {}/{} for '{}' ({:.4}s IO)",
        current_page, total_pages, db_name, io_sec
    );

    Ok(GetIndexDetailsResponse {
        source: source.to_string(),
        database: db_name.to_string(),
        total_pages,
        current_page,
        entries,
    })
}
