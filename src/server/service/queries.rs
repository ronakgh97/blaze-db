#[allow(unused)]
use crate::core::{HNSW, Metrics, NodeId};
#[allow(unused)]
use crate::prelude::{Provider, SearchQuery};
use crate::server::controller::INDEX_CACHE;
use crate::server::dto::QueryResult;
use crate::server::service::load_embeddings_index_from_database;
use crate::server::{QueryRequest, QueryResponse};
use crate::{error, info};
use anyhow::Result;
use std::sync::Arc;

/// Executes a search query against the specified database and returns the top K similar chunks.
pub async fn query_search(request: QueryRequest) -> Result<QueryResponse> {
    let query = &request.query;
    let source = &request.source;
    let from_database = &request.database;

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

    // TODO: Complete this
    // Check cache first, if fails load from disk and update cache :) Simple!! :)
    let cache_key = format!("{}_{}", &request.database, &request.source);
    let _hnsw_index = {
        let mut cache = INDEX_CACHE.write().await;

        if let Some(cached) = cache.get(&cache_key) {
            info!("Cache HIT for database '{}'", request.database);
            cached.clone()
        } else {
            info!("Cache MISS for database '{}'", request.database);

            // Load from disk
            let (store, _) = load_embeddings_index_from_database(
                request.database.clone(),
                request.source.clone(),
            )
            .await;

            let store = match store {
                Some(s) => Arc::new(s),
                None => return Err(anyhow::anyhow!("No index found")),
            };

            // Add to cache
            cache.put(cache_key.clone(), store.clone());
            store
        }
    };

    let (embeddings_store, _max_index) =
        load_embeddings_index_from_database(from_database.clone(), source.clone()).await; // TODO: Should preload the index at startup or something else, Like TTL caching
    let hnsw_index = match embeddings_store {
        Some(store) => store.hnsw_store,
        None => {
            error!("No embeddings found in database '{}'", from_database);
            return Err(anyhow::anyhow!(
                "No embeddings found in database '{}'",
                from_database
            ));
        }
    };

    info!("Loaded HNSW Index with {} entries", hnsw_index.nodes.len());

    info!(
        "Performing search with Cosine metric (top_k={})",
        request.top_k
    );

    let start_time = std::time::Instant::now();
    let result: Vec<(NodeId, f32, &str)> =
        HNSW::search_with_metadata(&hnsw_index, &query_vector, request.top_k);
    let duration_ms = start_time.elapsed().as_secs_f64();
    info!(
        "Search complete in {}s , found {} results",
        duration_ms,
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
        time_ms: duration_ms,
    };

    Ok(response)
}
