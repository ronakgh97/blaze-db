use blaze_db::prelude::{EmbeddingStore, HNSW, Ingestor, Provider};
use colored::Colorize;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::init(url, model);

    let batch_size = 1024;
    let ingestor = Ingestor::new("./sample/War_and_peace.txt", batch_size);

    match ingestor.read_chunks(150, 50) {
        Ok(batched_data) => {
            let total_chunks: usize = batched_data.par_iter().map(|b| b.len()).sum();
            println!();
            println!("Batch size: {}", batch_size.to_string().cyan());
            println!("Total batches: {}", batched_data.len().to_string().blue());
            println!("Total chunks: {}", total_chunks.to_string().green());
            println!("{}", "Processing embeddings...".yellow());
            println!();

            // Create HNSW index with optimal parameters
            // max_neighbors: 18, ef_construction: 200, max_layers: 12, distribution_bias: 0.8
            let mut hnsw = HNSW::new(18, 200, 12, 0.8);

            for (index, chunk) in batched_data.iter().enumerate() {
                match provider.fetch_embeddings(chunk).await {
                    Ok(embeddings) => {
                        let embedded_count = embeddings.embedding.len();

                        // Insert each embedding into HNSW index
                        for (i, vector) in embeddings.embedding.iter().enumerate() {
                            let random_level = hnsw.get_random_level();
                            let metadata = chunk.get(i).cloned().unwrap_or("[EMPTY]".to_string());
                            hnsw.insert(vector.clone(), metadata, random_level);
                        }

                        let mut embedding_store = EmbeddingStore::new(hnsw.clone());
                        let filename = PathBuf::from(format!("./embeddings/index_batch_{}", index));

                        if let Err(e) = embedding_store.write_to_disk(&filename).await {
                            eprintln!("Failed to write embeddings to file: {}", e);
                        } else {
                            println!(
                                "Batch {} saved ({} vectors, {} nodes in HNSW)",
                                index.to_string().green(),
                                embedded_count,
                                hnsw.nodes.len()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error fetching embeddings: {}", e);
                    }
                }
            }

            println!();
            println!(
                "Total chunks embedded: {}",
                total_chunks.to_string().bright_green()
            );
            println!(
                "Final HNSW index size: {} nodes",
                hnsw.nodes.len().to_string().bright_cyan()
            );
        }

        Err(e) => {
            eprintln!("Error reading chunks: {}", e);
        }
    }
}
