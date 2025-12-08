use blaze_db::prelude::{EmbeddingStore, Ingestor, Provider};
use colored::Colorize;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

#[tokio::main]
async fn main() {
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::new(url, model);

    let batch_size = 768;
    let ingestor = Ingestor::new("./sample/War_and_peace.txt", batch_size);

    match ingestor.read_line() {
        Ok(batched_data) => {
            let total_lines: usize = batched_data.par_iter().map(|b| b.len()).sum();
            println!();
            println!("Batch size: {}", batch_size.to_string().cyan());
            println!("Total batch: {}", batched_data.len().to_string().blue());
            println!("Total Lines: {}", total_lines.to_string().green());
            println!();

            println!();

            for (index, chunk) in batched_data.iter().enumerate() {
                match provider.fetch_embeddings(chunk).await {
                    Ok(embeddings) => {
                        let embedding_store = EmbeddingStore::new(index, embeddings.data);
                        embedding_store.debug_print();
                        let filename = format!("./embeddings/embeddings_batch_{}", index);
                        if let Err(e) = embedding_store.write_binary(&filename).await {
                            eprintln!("Failed to write embeddings to file: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error fetching embeddings: {}", e);
                    }
                }
            }
        }

        Err(e) => {
            eprintln!("Error reading lines: {}", e);
        }
    }
}
