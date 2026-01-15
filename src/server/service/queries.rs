#[allow(unused)]
use crate::core::{HNSW, Metrics, NodeId};
#[allow(unused)]
use crate::prelude::{Provider, SearchQuery};
use crate::server::service::load_embeddings_index_from_database;
use crate::server::{QueryRequest, QueryResponse};
use crate::{error, info};
use anyhow::Result;

/// Executes a search query against the specified database and returns the top K similar chunks.
pub async fn query_search(request: QueryRequest) -> Result<Vec<QueryResponse>> {
    let query = request.query;
    let from_database = request.database;

    //TODO: Provider should not be set on the fly
    // Configure embedding provider
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::init(url, model);

    info!("Generating embedding for query: '{}'", query);

    // Generate embedding for query
    // TODO: Maybe take vector for explicit
    let query_vector = &provider.fetch_embedding(query.as_str()).await?.embedding[0];

    info!("Loading vector data from database '{}'", from_database);
    let (embeddings_store, _max_index) =
        load_embeddings_index_from_database(from_database.clone()).await; //TODO: Should preload the index at startup
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

    let result: Vec<(NodeId, f32, String)> =
        HNSW::search_with_metadata(&hnsw_index, &query_vector, request.top_k);
    info!("Search complete, found {} results", result.len());

    // Map SearchResult to QueryResponse
    let responses = result
        .into_iter()
        .map(|r| QueryResponse {
            chunk: r.2.to_string(),
            score: r.1,
        })
        .collect();

    Ok(responses)
}
