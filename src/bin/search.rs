use blaze_db::prelude::*;
use colored::Colorize;
use tokio::time::Instant;

#[tokio::main]
pub async fn main() {
    let sample_text = String::from("What is this about?");

    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
    );

    match provider.fetch_embedding(&sample_text).await {
        Ok(embeddings) => {
            println!("Query: {}", sample_text);
            println!("Embedding (First 3): {:?}", &embeddings.embedding[0][..3]);
            println!();

            let io_start = Instant::now();
            // Find the latest index file in embeddings directory
            let (embeddings_store, index) =
                EmbeddingStore::load_lastest_index("index_batch", "./embeddings")
                    .await
                    .expect("Failed to load embeddings from directory");

            let store = embeddings_store.expect("Failed to load index file");
            println!("Lastest index file loaded: {}", index.to_string().cyan());
            let io_duration = io_start.elapsed();

            let hnsw = &store.hnsw_store;
            println!("Checksum: {}", store.checksum.to_string().red());
            println!("Loaded HNSW index with {} nodes", hnsw.nodes.len());
            println!(
                "Index parameters: M={}, ef_construction={}, layers={}",
                hnsw.max_neighbors, hnsw.ef_construction, hnsw.max_layers
            );
            println!();

            let search_start = Instant::now();
            let query_vector = &embeddings.embedding[0];
            let top_k = 5;

            // Use HNSW search
            let results = hnsw.search_with_metadata(query_vector, top_k);
            let search_duration = search_start.elapsed();

            println!("Top {} similar chunks (HNSW):", top_k);
            for (_i, (node_id, similarity, metadata)) in results.iter().enumerate() {
                println!();
                println!("Node ID: {}", node_id.to_string().cyan());
                println!("Similarity: {:.4}", similarity.to_string().yellow());
                println!("Vector (first 5): {:?}", &hnsw.nodes[*node_id].vector[..5]);
                println!("Metadata: {}", metadata.to_string().green().dimmed());
            }

            let total_duration = io_start.elapsed();
            println!(
                "\nI/O took: {:?} to load {} nodes",
                io_duration,
                hnsw.nodes.len()
            );
            println!(
                "HNSW search took: {:?} for {} nodes",
                search_duration,
                hnsw.nodes.len()
            );
            println!("Total took: {:?}", total_duration);
        }
        Err(e) => {
            eprintln!("Error fetching embeddings: {}", e);
        }
    }
}
