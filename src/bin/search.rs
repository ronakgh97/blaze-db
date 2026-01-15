use blaze_db::prelude::*;
use colored::Colorize;
use std::path::PathBuf;
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

            // Find the latest index file in embeddings directory
            let embeddings_dir = PathBuf::from("./embeddings");
            let mut entries = tokio::fs::read_dir(&embeddings_dir)
                .await
                .expect("Failed to read embeddings directory");

            let mut latest_file: Option<PathBuf> = None;
            let mut latest_number: usize = 0;

            while let Some(entry) = entries.next_entry().await.unwrap() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        // Extract number from filename like "embeddings_batch_23"
                        if let Some(num_str) = filename.strip_prefix("embeddings_batch_") {
                            if let Ok(num) = num_str.parse::<usize>() {
                                if num > latest_number {
                                    latest_number = num;
                                    latest_file = Some(path.clone());
                                }
                            }
                        }
                    }
                }
            }

            let latest_index = latest_file.expect("No embedding files found in ./embeddings");
            println!("Loading latest index: {:?}", latest_index.file_name());
            let start = Instant::now();
            let store = EmbeddingStore::load_binary_file(&latest_index)
                .await
                .expect("Failed to load index file");
            let io_duration = start.elapsed();

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

            let total_duration = start.elapsed();
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
