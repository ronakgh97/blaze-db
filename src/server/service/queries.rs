use crate::core::Metrics;
use crate::prelude::{Provider, SearchQuery};
use crate::server::service::read_embeddings_from_database;
use crate::server::{QueryRequest, QueryResponse};
use anyhow::Result;

/// Executes a search query against the specified database and returns the top K similar chunks.
pub async fn query_search(request: QueryRequest) -> Result<Vec<QueryResponse>> {
    let query = request.query;
    let from_database = request.database;

    //TODO: Provider should not be set on the fly
    // Configure embedding provider
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::new(url, model);

    // Take the first as it's a single text query, we unwrap safely here so (Help me GOD!!)
    let query_vector = provider
        .fetch_embedding(query.as_str())
        .await?
        .data
        .first()
        .unwrap()
        .embedding
        .clone();

    let vector_data = read_embeddings_from_database(from_database).await?;

    let search = SearchQuery::new(request.top_k, query_vector, Metrics::Cosine);

    let results = search.search(&vector_data);

    // Map SearchResult to QueryResponse
    let responses = results
        .into_iter()
        .map(|r| QueryResponse {
            chunk: r.chunk,
            score: r.score,
        })
        .collect();

    Ok(responses)
}
