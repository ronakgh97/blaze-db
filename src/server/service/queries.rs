#[allow(unused)]
use crate::core::{HNSW, Metrics, NodeIndex, SERVER_FILE};
#[allow(unused)]
use crate::prelude::Provider;
use crate::server::controller::{ErrorTypes, INDEX_CACHE, LOADING_LOCKS};
use crate::server::dto::{QueryResult, VectorQueryRequest, VectorQueryResponse, VectorQueryResult};
use crate::server::service::database::search_database_on_disk;
use crate::server::service::load_index_from_database;
use crate::server::{QueryRequest, QueryResponse, VectorDataDto};
use crate::utils::read_embeddings_metadata;
use crate::{debug, error, info, trace, warn};
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

// This is the threshold for when we consider the database "small enough" to skip the HNSW search and do a brute-force search instead.
// This is a very rough heuristic and should be tuned based on benchmarking with real data and hardware.
// For now, lets just do (N * D) where N is the number of vectors and D is the dimension
// Anyway, this is still bad, cuz CPU and RAM are factors, maybe later, we can implement a usage-pattern-based smart search strategy.
const BRUTE_SEARCH_THRESHOLD: usize = 25_000_000;

pub async fn query_vector(
    request: VectorQueryRequest,
    _provider: &Provider,
) -> Result<VectorQueryResponse> {
    let vector_query = &request.query_vector;
    let source = &request.source;
    let from_database = &request.database;

    // Get database directory path
    let db_path = search_database_on_disk(&from_database, &source)
        .await
        .map_err(|e| {
            error!(
                "Database '{}' not found in source '{}'",
                from_database, source
            );
            ErrorTypes::DatabaseNotFound(format!(
                "Database '{}' not found in source '{}': {}",
                from_database, source, e
            ))
        })?;

    info!("Received vector query for database '{}'", from_database);

    let io_time_start = Instant::now();

    // Check cache with read lock (allows concurrent reads)
    let cache_key = format!("{}_{}", from_database, source);

    {
        let cache = INDEX_CACHE.read().await;
        if let Some(cached) = cache.peek(&cache_key) {
            debug!("Cache HIT for database '{}'", from_database);

            // Validate cache by comparing checksums
            let (checksum_on_disk, dimensions, node_count) =
                match read_embeddings_metadata(&db_path).await {
                    Ok(meta) => (meta.checksum, meta.dimensions, meta.total_vectors),
                    Err(e) => {
                        error!(
                            "Failed to read embeddings metadata for cache validation: {}",
                            e
                        );
                        return Err(ErrorTypes::IndexNotFound(format!(
                            "Failed to validate cache, index not found, Error: {}",
                            e
                        ))
                        .into());
                    }
                };

            if cached.0.checksum == checksum_on_disk {
                debug!("Cache is valid for database '{}'", from_database);

                let io_duration_sec = io_time_start.elapsed().as_secs_f64();
                let (_metadata, hnsw_index) = (cached.0.clone(), cached.1.clone());

                debug!(
                    "Loaded HNSW Index with {} entries",
                    hnsw_index.hnsw_store.nodes.len()
                );

                debug!(
                    "Performing vector search with provided embedding (top_k={})",
                    request.top_k
                );

                let start_time = Instant::now();

                let result: Vec<(String, f32, String)> = if dimensions * node_count
                    <= BRUTE_SEARCH_THRESHOLD
                {
                    debug!(
                        "Database is small ({} vectors with {} dimensions), performing brute-force search",
                        node_count, dimensions
                    );
                    HNSW::brute_force_search_with_metadata(
                        &hnsw_index.hnsw_store,
                        vector_query,
                        request.top_k,
                    )
                } else {
                    HNSW::search_with_metadata(
                        &hnsw_index.hnsw_store,
                        vector_query,
                        request.top_k,
                        None,
                    )
                };

                let duration_sec = start_time.elapsed().as_secs_f64();
                debug!(
                    "Search complete in {}s, found {} results",
                    duration_sec,
                    result.len()
                );

                let result_map = result
                    .into_iter()
                    .map(|r| VectorQueryResult {
                        vectordata: VectorDataDto {
                            id: r.0.clone(),
                            embedding: HNSW::get_node_by_id(&hnsw_index.hnsw_store, &*r.0)
                                .unwrap()
                                .vector
                                .clone(), // Just pray here for to unwrap, since the ID should exist in the store
                            metadata: r.2.to_string(),
                        },
                        score: r.1,
                    })
                    .collect();

                let response = VectorQueryResponse {
                    results: result_map,
                    search_time_sec: duration_sec,
                    io_time_sec: io_duration_sec,
                };

                // Update last_accessed_at in SERVER_FILE
                {
                    let mut server_file = SERVER_FILE.write().await;
                    if let Err(e) = server_file.touch_vector_base(source, from_database) {
                        warn!(
                            "Failed to update last_accessed_at for database '{}': {}",
                            from_database, e
                        );
                    }
                }

                return Ok(response);
            }

            debug!(
                "Cache is stale for database '{}', will reload from disk",
                from_database
            );
        }
    }

    // Cache miss or stale - need to load from disk
    debug!("Cache MISS for database '{}'", from_database);

    // Get per-database loading lock to prevent duplicate loads
    let loading_lock = {
        let mut locks = LOADING_LOCKS.write().await;
        locks
            .get_or_insert_mut(cache_key.clone(), || Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };

    // Acquire loading lock for this specific database
    // This ensures only one thread loads this database at a time
    let _load_guard = loading_lock.lock().await;
    trace!("Acquired loading lock for database '{}'", from_database);

    {
        let cache = INDEX_CACHE.read().await;
        if let Some(cached) = cache.peek(&cache_key) {
            let checksum_on_disk = match read_embeddings_metadata(&db_path).await {
                Ok(meta) => meta.checksum,
                Err(e) => {
                    error!(
                        "Failed to read embeddings metadata for cache validation: {}",
                        e
                    );
                    return Err(ErrorTypes::IndexNotFound(format!(
                        "Failed to validate cache, index not found, Error: {}",
                        e
                    ))
                    .into());
                }
            };

            if cached.0.checksum == checksum_on_disk {
                debug!(
                    "Cache HIT after waiting for concurrent load of database '{}'",
                    from_database
                );

                let io_duration_sec = io_time_start.elapsed().as_secs_f64();
                let (_metadata, hnsw_index) = (cached.0.clone(), cached.1.clone());

                debug!(
                    "Loaded HNSW Index with {} entries",
                    hnsw_index.hnsw_store.nodes.len()
                );

                debug!(
                    "Performing vector search with provided embedding (top_k={})",
                    request.top_k
                );

                // Don't do brute-force search, since we are loading the index
                // We need to be fast here, no matter the accuracy on first run ;)
                let start_time = Instant::now();
                let result: Vec<(String, f32, String)> = HNSW::search_with_metadata(
                    &hnsw_index.hnsw_store,
                    vector_query,
                    request.top_k,
                    None,
                );
                let duration_sec = start_time.elapsed().as_secs_f64();
                debug!(
                    "Search complete in {}s, found {} results",
                    duration_sec,
                    result.len()
                );

                let result_map = result
                    .into_iter()
                    .map(|r| VectorQueryResult {
                        vectordata: VectorDataDto {
                            id: r.0.clone(),
                            embedding: HNSW::get_node_by_id(&hnsw_index.hnsw_store, &*r.0)
                                .unwrap()
                                .vector
                                .clone(), // Just pray here for to unwrap, since the ID should exist in the store
                            metadata: r.2.to_string(),
                        },
                        score: r.1,
                    })
                    .collect();

                let response = VectorQueryResponse {
                    results: result_map,
                    search_time_sec: duration_sec,
                    io_time_sec: io_duration_sec,
                };

                // Update last_accessed_at in SERVER_FILE
                {
                    let mut server_file = SERVER_FILE.write().await;
                    if let Err(e) = server_file.touch_vector_base(source, from_database) {
                        warn!(
                            "Failed to update last_accessed_at for database '{}': {}",
                            from_database, e
                        );
                    }
                }

                return Ok(response);
            }
        }
    }

    // Still missing or stale - load from disk (only one thread per database does this)
    debug!("Loading index from disk for database '{}'", from_database);
    let store = load_index_from_database(from_database.clone(), source.clone()).await;

    // Read the metadata
    let metadata = match read_embeddings_metadata(&db_path).await {
        Ok(meta) => Arc::new(meta),
        Err(e) => {
            error!("Failed to read embeddings metadata: {}", e);
            return Err(ErrorTypes::IndexNotFound(format!(
                "Failed to read embeddings metadata: {}",
                e
            ))
            .into());
        }
    };

    // Get the store or return error
    let store = match store {
        Some(s) => Arc::new(s),
        None => {
            return Err(ErrorTypes::IndexNotFound("No index found in database".to_string()).into());
        }
    };

    // Update cache with minimal write lock time
    {
        let mut cache = INDEX_CACHE.write().await;
        cache.put(cache_key, (Arc::clone(&metadata), Arc::clone(&store)));
    }

    trace!("Released loading lock for database '{}'", from_database);

    let io_duration_sec = io_time_start.elapsed().as_secs_f64();
    debug!("I/O operations for loading index took {}s", io_duration_sec);

    let hnsw_index = store;

    debug!(
        "Loaded HNSW Index with {} entries",
        hnsw_index.hnsw_store.nodes.len()
    );

    debug!(
        "Performing vector search with provided embedding (top_k={})",
        request.top_k
    );

    // Same here
    let start_time = Instant::now();
    let result: Vec<(String, f32, String)> =
        HNSW::search_with_metadata(&hnsw_index.hnsw_store, vector_query, request.top_k, None);
    let duration_sec = start_time.elapsed().as_secs_f64();
    info!(
        "Search complete in {}s, found {} results",
        duration_sec,
        result.len()
    );

    // Map SearchResult to VectorQueryResponse
    let result_map = result
        .into_iter()
        .map(|r| VectorQueryResult {
            vectordata: VectorDataDto {
                id: r.0.clone(),
                embedding: HNSW::get_node_by_id(&hnsw_index.hnsw_store, &*r.0)
                    .unwrap()
                    .vector
                    .clone(), // Just pray here for to unwrap, since the ID should exist in the store
                metadata: r.2.to_string(),
            },
            score: r.1,
        })
        .collect();

    let response = VectorQueryResponse {
        results: result_map,
        search_time_sec: duration_sec,
        io_time_sec: io_duration_sec,
    };

    // Update last_accessed_at in SERVER_FILE
    {
        let mut server_file = SERVER_FILE.write().await;
        if let Err(e) = server_file.touch_vector_base(source, from_database) {
            warn!(
                "Failed to update last_accessed_at for database '{}': {}",
                from_database, e
            );
            // Don't fail the query - metadata update is not critical
        }
    }

    Ok(response)
}

/// Executes a search query against the specified database and returns the top K similar chunks.
pub async fn query_search(request: QueryRequest, provider: &Provider) -> Result<QueryResponse> {
    let query = &request.query;
    let source = &request.source;
    let from_database = &request.database;

    // Get database directory path
    let db_path = search_database_on_disk(&from_database, &source)
        .await
        .map_err(|e| {
            error!(
                "Database '{}' not found in source '{}'",
                from_database, source
            );
            ErrorTypes::DatabaseNotFound(format!(
                "Database '{}' not found in source '{}': {}",
                from_database, source, e
            ))
        })?;

    info!("Generating embedding for query: '{}'", query);

    // Generate embedding for query
    let query_vector = &provider.fetch_embedding(query.as_str()).await?.embedding[0];

    // info!("Loading vector data from database '{}'", from_database);

    let io_time_start = Instant::now();

    // Check cache with read lock (allows concurrent reads)
    let cache_key = format!("{}_{}", &request.database, &request.source);

    {
        let cache = INDEX_CACHE.read().await;
        if let Some(cached) = cache.peek(&cache_key) {
            debug!("Cache HIT for database '{}'", request.database);

            // Validate cache by comparing checksums
            let (checksum_on_disk, dimension, vector_count) =
                match read_embeddings_metadata(&db_path).await {
                    Ok(meta) => (meta.checksum, meta.dimensions, meta.total_vectors),
                    Err(e) => {
                        error!(
                            "Failed to read embeddings metadata for cache validation: {}",
                            e
                        );
                        return Err(ErrorTypes::IndexNotFound(format!(
                            "Failed to validate cache, index not found, Error: {}",
                            e
                        ))
                        .into());
                    }
                };

            if cached.0.checksum == checksum_on_disk {
                debug!("Cache is valid for database '{}'", request.database);

                let io_duration_sec = io_time_start.elapsed().as_secs_f64();
                let (_metadata, hnsw_index) = (cached.0.clone(), cached.1.clone());

                debug!(
                    "Loaded HNSW Index with {} entries",
                    hnsw_index.hnsw_store.nodes.len()
                );

                debug!(
                    "Performing search with Cosine metric (top_k={})",
                    request.top_k
                );

                let start_time = Instant::now();

                // Perform Brute-force search if, apply, this is fine, since its already in loaded
                let result: Vec<(String, f32, String)> = if dimension * vector_count
                    <= BRUTE_SEARCH_THRESHOLD
                {
                    debug!(
                        "Database is small ({} vectors with {} dimensions), performing brute-force search",
                        vector_count, dimension
                    );
                    HNSW::brute_force_search_with_metadata(
                        &hnsw_index.hnsw_store,
                        query_vector,
                        request.top_k,
                    )
                } else {
                    HNSW::search_with_metadata(
                        &hnsw_index.hnsw_store,
                        query_vector,
                        request.top_k,
                        None,
                    )
                };

                let duration_sec = start_time.elapsed().as_secs_f64();
                debug!(
                    "Search complete in {}s , found {} results",
                    duration_sec,
                    result.len()
                );

                let result_map = result
                    .into_iter()
                    .map(|r| QueryResult {
                        id: r.0,
                        chunk: r.2.to_string(),
                        score: r.1,
                    })
                    .collect();

                let response = QueryResponse {
                    results: result_map,
                    search_time_sec: duration_sec,
                    io_time_sec: io_duration_sec,
                };

                // Update last_accessed_at in SERVER_FILE
                {
                    let mut server_file = SERVER_FILE.write().await;
                    if let Err(e) =
                        server_file.touch_vector_base(&request.source, &request.database)
                    {
                        warn!(
                            "Failed to update last_accessed_at for database '{}': {}",
                            request.database, e
                        );
                    }
                }

                return Ok(response);
            }

            debug!(
                "Cache is stale for database '{}', will reload from disk",
                request.database
            );
        }
    }

    // Cache miss or stale - need to load from disk
    debug!("Cache MISS for database '{}'", request.database);

    // Get per-database loading lock to prevent duplicate loads
    let loading_lock = {
        let mut locks = LOADING_LOCKS.write().await;
        locks
            .get_or_insert_mut(cache_key.clone(), || Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };

    // Acquire loading lock for this specific database
    let _load_guard = loading_lock.lock().await;
    trace!("Acquired loading lock for database '{}'", request.database);

    // Double-check cache (another thread might have loaded it while we waited)
    {
        let cache = INDEX_CACHE.read().await;
        if let Some(cached) = cache.peek(&cache_key) {
            let checksum_on_disk = match read_embeddings_metadata(&db_path).await {
                Ok(meta) => meta.checksum,
                Err(e) => {
                    error!(
                        "Failed to read embeddings metadata for cache validation: {}",
                        e
                    );
                    return Err(ErrorTypes::IndexNotFound(format!(
                        "Failed to validate cache, index not found, Error: {}",
                        e
                    ))
                    .into());
                }
            };

            if cached.0.checksum == checksum_on_disk {
                info!(
                    "Cache HIT after waiting for concurrent load of database '{}'",
                    request.database
                );

                let io_duration_sec = io_time_start.elapsed().as_secs_f64();
                let (_metadata, hnsw_index) = (cached.0.clone(), cached.1.clone());

                debug!(
                    "Loaded HNSW Index with {} entries",
                    hnsw_index.hnsw_store.nodes.len()
                );

                debug!(
                    "Performing search with Cosine metric (top_k={})",
                    request.top_k
                );

                // Same apply here for brute-force search, since we are loading the index, we want to be fast on first run, no matter the accuracy
                let start_time = Instant::now();
                let result: Vec<(String, f32, String)> = HNSW::search_with_metadata(
                    &hnsw_index.hnsw_store,
                    &query_vector,
                    request.top_k,
                    None,
                );
                let duration_sec = start_time.elapsed().as_secs_f64();
                debug!(
                    "Search complete in {}s , found {} results",
                    duration_sec,
                    result.len()
                );

                let result_map = result
                    .into_iter()
                    .map(|r| QueryResult {
                        id: r.0,
                        chunk: r.2.to_string(),
                        score: r.1,
                    })
                    .collect();

                let response = QueryResponse {
                    results: result_map,
                    search_time_sec: duration_sec,
                    io_time_sec: io_duration_sec,
                };

                // Update last_accessed_at in SERVER_FILE
                {
                    let mut server_file = SERVER_FILE.write().await;
                    if let Err(e) =
                        server_file.touch_vector_base(&request.source, &request.database)
                    {
                        warn!(
                            "Failed to update last_accessed_at for database '{}': {}",
                            request.database, e
                        );
                    }
                }

                return Ok(response);
            }
        }
    }

    // Still missing or stale - load from disk (only one thread per database does this)
    debug!(
        "Loading index from disk for database '{}'",
        request.database
    );
    let store = load_index_from_database(request.database.clone(), request.source.clone()).await;

    // Read the metadata
    let metadata = match read_embeddings_metadata(&db_path).await {
        Ok(meta) => Arc::new(meta),
        Err(e) => {
            error!("Failed to read embeddings metadata: {}", e);
            return Err(ErrorTypes::IndexNotFound(format!(
                "Failed to read embeddings metadata: {}",
                e
            ))
            .into());
        }
    };

    // Get the store or return error
    let store = match store {
        Some(s) => Arc::new(s),
        None => {
            return Err(ErrorTypes::IndexNotFound("No index found in database".to_string()).into());
        }
    };

    // Update cache with minimal write lock time
    {
        let mut cache = INDEX_CACHE.write().await;
        cache.put(
            cache_key.clone(),
            (Arc::clone(&metadata), Arc::clone(&store)),
        );
    }

    trace!("Released loading lock for database '{}'", request.database);

    let io_duration_sec = io_time_start.elapsed().as_secs_f64();
    debug!("I/O operations for loading index took {}s", io_duration_sec);

    let (_metadata, hnsw_index) = (metadata, store);

    debug!(
        "Loaded HNSW Index with {} entries",
        hnsw_index.hnsw_store.nodes.len()
    );

    debug!(
        "Performing search with Cosine metric (top_k={})",
        request.top_k
    );

    let start_time = Instant::now();
    let result: Vec<(String, f32, String)> =
        HNSW::search_with_metadata(&hnsw_index.hnsw_store, &query_vector, request.top_k, None);
    let duration_sec = start_time.elapsed().as_secs_f64();
    debug!(
        "Search complete in {}s , found {} results",
        duration_sec,
        result.len()
    );

    // Map SearchResult to QueryResponse
    let result_map = result
        .into_iter()
        .map(|r| QueryResult {
            id: r.0,
            chunk: r.2.to_string(),
            score: r.1,
        })
        .collect();

    let response = QueryResponse {
        results: result_map,
        search_time_sec: duration_sec,
        io_time_sec: io_duration_sec,
    };

    // TODO: Consider async/background task
    {
        let mut server_file = SERVER_FILE.write().await;
        if let Err(e) = server_file.touch_vector_base(&request.source, &request.database) {
            warn!(
                "Failed to update last_accessed_at for database '{}': {}",
                request.database, e
            );
            // Don't fail the query - metadata update is not critical
        }
    }

    Ok(response)
}
