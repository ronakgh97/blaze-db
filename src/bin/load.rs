use blaze_db::prelude::EmbeddingStore;
use colored::Colorize;

#[tokio::main]
async fn main() {
    println!();

    match EmbeddingStore::load_binaries("./embeddings").await {
        Ok(mut stores) => {
            println!(
                "{}",
                "Successfully loaded indexed embeddings".green().bold()
            );
            println!();
            println!("{}", "Stats:".yellow().bold());

            // Sort stores by number of nodes ascendingly to find the latest/largest index
            stores.sort_by_key(|s| s.hnsw_store.nodes.len());

            let total_batches = stores.len();
            let lastest_index = stores.last().unwrap();
            let checksum = stores.last().unwrap().checksum.clone();

            println!(
                " Total batches loaded: {}",
                total_batches.to_string().cyan()
            );
            println!(
                " Total nodes in HNSW (latest index): {}\n Checksum: {}",
                lastest_index.hnsw_store.nodes.len().to_string().cyan(),
                checksum.red()
            );

            let hnsw = &lastest_index.hnsw_store;
            println!(
                " HNSW max_neighbors: {}",
                hnsw.max_neighbors.to_string().cyan()
            );
            println!(
                " HNSW ef_construction: {}",
                hnsw.ef_construction.to_string().cyan()
            );
            println!(" HNSW max_layers: {}", hnsw.max_layers.to_string().cyan());

            if let Some(first_node) = hnsw.nodes.first() {
                println!(
                    " Vector dimensions: {}",
                    first_node.vector.len().to_string().cyan()
                );
            }

            println!();

            // Display sample nodes from largest store
            println!(" {}", "Sample Nodes (first 3):".yellow().bold());

            for (_idx, node) in hnsw.nodes.iter().take(3).enumerate() {
                println!(" \nNode ID: {}", node.id.to_string().cyan());
                println!(" Max Level: {}", node.max_level);
                println!(
                    " Neighbors per layer: {:?}",
                    node.neighbors.iter().map(|n| n.len()).collect::<Vec<_>>()
                );
                println!(
                    " Vector (first 5): {:?}",
                    &node.vector[..5.min(node.vector.len())]
                );
                println!(" Vector dimensions: {}", node.vector.len());
                println!("\nMetadata: {}", node.metadata.to_string().green().dimmed());
            }
        }
        Err(e) => {
            eprintln!("{}", "Failed to load embeddings".red().bold());
            eprintln!("Error: {}", e);
        }
    }
}
