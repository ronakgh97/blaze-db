use blaze_db::prelude::*;
use tokio::time::Instant;
#[tokio::main]
pub async fn main() {
    let sample_text = String::from("What this book about?");

    let provider = Provider::new(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
    );

    match provider.fetch_embedding(&sample_text).await {
        Ok(embeddings) => {
            for embedding in embeddings.data.clone() {
                println!("Chunk: {}", &embedding.chunk);
                println!("Embedding (First 3): {:?}", &embedding.embedding[..3]);
            }

            let start = Instant::now();
            let vector_data = EmbeddingStore::read_binary("./embeddings").await.unwrap();
            let io_duration = start.elapsed();

            let search_start = Instant::now();
            let search_query =
                SearchQuery::new(5, embeddings.data[0].embedding.clone(), Metrics::Cosine);

            let result = search_query.search_vector(&vector_data);
            let search_duration = search_start.elapsed();

            println!("\nTop {} similar chunks:", search_query.top_k);
            for (i, item) in result.iter().enumerate() {
                println!("\nResult {}:", i + 1);
                println!("Chunk: {}", item.chunk);
                println!("Score: {:.4}", item.score);
            }

            let total_duration = start.elapsed();
            println!(
                "\nI/O took: {:?} for {} vectors",
                io_duration, vector_data.total_vectors
            );
            println!(
                "Search took: {:?} for {} vectors",
                search_duration, vector_data.total_vectors
            );
            println!("Total took: {:?}", total_duration);
        }
        Err(e) => {
            eprintln!("Error fetching embeddings: {}", e);
        }
    }
}
