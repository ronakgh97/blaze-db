#[allow(unused)]
use crate::core::{HNSW, Metrics, NodeId};
#[allow(unused)]
use crate::prelude::{Provider, SearchQuery};
use crate::server::controller::INDEX_CACHE;
use crate::server::dto::QueryResult;
use crate::server::service::database::search_database;
use crate::server::service::load_embeddings_index_from_database;
use crate::server::{QueryRequest, QueryResponse};
use crate::utils::read_embeddings_metadata;
use crate::{error, info};
use anyhow::Result;
use std::sync::Arc;

/// Executes a search query against the specified database and returns the top K similar chunks.
pub async fn query_search(request: QueryRequest) -> Result<QueryResponse> {
    let query = &request.query;
    let source = &request.source;
    let from_database = &request.database;

    // Get database directory path
    let db_path = search_database(from_database.clone(), source.clone()).await?;

    // Configure embedding provider from env or use defaults
    let url = std::env::var("EMBEDDING_API_URL")
        .unwrap_or_else(|_| "http://localhost:1234/v1/embeddings".to_string());
    let model = std::env::var("EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-qwen3-embedding-0.6b".to_string());
    let provider = Provider::init(url, model);

    info!("Generating embedding for query: '{}'", query);

    // Generate embedding for query
    // TODO: Maybe take vector for explicit
    let query_vector = &provider.fetch_embedding(query.as_str()).await?.embedding[0];

    // info!("Loading vector data from database '{}'", from_database);

    let io_time_start = std::time::Instant::now();
    // Check cache first, if fails load from disk and update cache using hash checks. Simple!! :)
    let cache_key = format!("{}_{}", &request.database, &request.source);
    #[allow(unused)]
    let cache_index = {
        let mut cache = INDEX_CACHE.write().await;

        if let Some(cached) = cache.get(&cache_key) {
            info!("Cache HIT for database '{}'", request.database);

            // Check the cache validation by comparing checksums or timestamps if needed
            let (metadata, store) = cached;

            // TODO: Still touching disk, not ideally cache (no I/O), but better than loading full index least 😊
            let checksum_on_disk = match read_embeddings_metadata(&db_path).await {
                Ok(meta) => meta.checksum,
                Err(e) => {
                    error!(
                        "Failed to read embeddings metadata for cache validation: {}",
                        e
                    );
                    return Err(anyhow::anyhow!("Failed to read embeddings metadata: {}", e));
                }
            };

            if metadata.checksum == checksum_on_disk {
                info!("Cache is valid for database '{}'", request.database);
                (metadata.clone(), store.clone())
            } else {
                info!(
                    "Cache is stale for database '{}', reloading from disk",
                    request.database
                );

                // Load from disk
                let (store, _) = load_embeddings_index_from_database(
                    request.database.clone(),
                    request.source.clone(),
                )
                .await;

                // Try read the metadata
                let metadata = match read_embeddings_metadata(&db_path).await {
                    Ok(meta) => Arc::new(meta),
                    Err(e) => {
                        error!("Failed to read embeddings metadata: {}", e);
                        return Err(anyhow::anyhow!("Failed to read embeddings metadata: {}", e));
                    }
                };

                // Try to get the store or return error
                let store = match store {
                    Some(s) => Arc::new(s),
                    None => return Err(anyhow::anyhow!("No index found")),
                };

                // Update cache
                cache.put(cache_key.clone(), (metadata.clone(), store.clone()));
                (metadata, store)
            }
        } else {
            info!("Cache MISS for database '{}'", request.database);

            // Load from disk
            let (store, _idx) = load_embeddings_index_from_database(
                request.database.clone(),
                request.source.clone(),
            )
            .await;

            // Try read the metadata
            let metadata = match read_embeddings_metadata(&db_path).await {
                Ok(meta) => Arc::new(meta),
                Err(e) => {
                    error!("Failed to read embeddings metadata: {}", e);
                    return Err(anyhow::anyhow!("Failed to read embeddings metadata: {}", e));
                }
            };

            // Try to get the store or return error
            let store = match store {
                Some(s) => Arc::new(s),
                None => return Err(anyhow::anyhow!("No index found")),
            };

            // Add to cache
            cache.put(cache_key.clone(), (metadata.clone(), store.clone()));
            (metadata, store)
        }
    };

    let io_duration_ms = io_time_start.elapsed().as_secs_f64();
    info!(
        "I/O operations for loading index or check cache took {}s",
        io_duration_ms
    );

    // let (embeddings_store, _max_index) =
    //     load_embeddings_index_from_database(from_database.clone(), source.clone()).await; // TODO: Should preload the index at startup or something else, Like TTL caching
    // let hnsw_index = match embeddings_store {
    //     Some(store) => store.hnsw_store,
    //     None => {
    //         error!("No embeddings found in database '{}'", from_database);
    //         return Err(anyhow::anyhow!(
    //             "No embeddings found in database '{}'",
    //             from_database
    //         ));
    //     }
    // };

    let (_metadata, hnsw_index) = cache_index;

    info!(
        "Loaded HNSW Index with {} entries",
        hnsw_index.hnsw_store.nodes.len()
    );

    info!(
        "Performing search with Cosine metric (top_k={})",
        request.top_k
    );

    let start_time = std::time::Instant::now();
    let result: Vec<(NodeId, f32, &str)> =
        HNSW::search_with_metadata(&hnsw_index.hnsw_store, &query_vector, request.top_k);
    let duration_sec = start_time.elapsed().as_secs_f64();
    info!(
        "Search complete in {}s , found {} results",
        duration_sec,
        result.len()
    );

    // Map SearchResult to QueryResponse
    let result_map = result
        .into_iter()
        .map(|r| QueryResult {
            chunk: r.2.to_string(),
            score: r.1,
        })
        .collect();

    let response = QueryResponse {
        results: result_map,
        time_sec: duration_sec,
    };

    Ok(response)
}
