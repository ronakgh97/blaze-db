use blaze_db::prelude::{EmbeddingStore, Ingestor, Provider};
use colored::Colorize;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

#[tokio::main]
async fn main() {
    let url = "http://localhost:1234/v1/embeddings";
    let model = "text-embedding-qwen3-embedding-0.6b";
    let provider = Provider::new(url, model);

    let batch_size = 256;
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

            for (index, chunk) in batched_data.iter().enumerate() {
                match provider.fetch_embeddings(chunk).await {
                    Ok(embeddings) => {
                        let embedding_store = EmbeddingStore::new(index, embeddings.data);
                        embedding_store.debug_print();
                        let filename = format!("./embeddings/embeddings_batch_{}", index);
                        if let Err(e) = embedding_store.write_binary(&filename).await {
                            eprintln!("Failed to write embeddings to file: {}", e);
                        } else {
                            println!("Batch {} saved", index.to_string().green());
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
        }

        Err(e) => {
            eprintln!("Error reading chunks: {}", e);
        }
    }
}
