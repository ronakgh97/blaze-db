use blaze_db::prelude::EmbeddingStore;
use colored::Colorize;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;

#[tokio::main]
async fn main() {
    println!();

    match EmbeddingStore::read_binary("./embeddings").await {
        Ok(vector_data) => {
            println!("{}", "Successfully loaded embeddings".green().bold());
            println!();
            println!("{}", "Stats:".yellow().bold());
            println!(
                " Total vectors: {}",
                vector_data.total_vectors.to_string().cyan()
            );
            println!(" Dimensions: {}", vector_data.dimensions.to_string().cyan());
            println!(
                " Total chunks: {}",
                vector_data.chunk.len().to_string().cyan()
            );

            // Calculate average chunk size
            if !vector_data.chunk.is_empty() {
                let total_words: usize = vector_data
                    .chunk
                    .par_iter()
                    .map(|c| c.split_whitespace().count())
                    .sum();
                let avg_words = total_words / vector_data.chunk.len();
                println!(" Avg chunk size: {} words", avg_words.to_string().cyan());
            }

            println!(" Memory Usage: {}MB", vector_data.memory_usage_mb());
            println!();

            // Display sample data
            if !vector_data.chunk.is_empty() {
                println!(" {}", "Sample Chunks (first 3):".yellow().bold());
                for (index, (chunk, embedding)) in vector_data
                    .chunk
                    .iter()
                    .zip(vector_data.embedding.iter())
                    .take(3)
                    .enumerate()
                {
                    println!();
                    println!("  {} {}", "Chunk".blue(), index + 1);
                    let preview = if chunk.len() > 50 {
                        format!("{}...", chunk.chars().take(50).collect::<String>())
                    } else {
                        chunk.clone()
                    };
                    println!("    Text: {}", preview.cyan());
                    println!("    Words: {}", chunk.split_whitespace().count());
                    println!(
                        "    Embedding (first 5): {:?}",
                        &embedding[..5.min(embedding.len())]
                    );
                    println!("    Dimensions: {}", embedding.len());
                }
            }
        }
        Err(e) => {
            eprintln!("{}", "Failed to load embeddings".red().bold());
            eprintln!("Error: {}", e);
        }
    }
}
