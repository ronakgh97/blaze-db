use blaze_db::prelude::*;
use tokio::time::Instant;
#[tokio::main]
pub async fn main() {
    let sample_text = "There is no Peace without War,\nWars should be celebrated,\nBecause it is the win against the evil.";
    let chunks = vec![sample_text.to_string()];

    let provider = Provider::new(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
    );

    match provider.fetch_embeddings(&chunks).await {
        Ok(embeddings) => {
            for embedding in embeddings.data.clone() {
                println!("Chunk: {}", &embedding.chunk);
                println!("Embedding (First 3): {:?}", &embedding.embedding[..3]);
            }

            let start = Instant::now();

            let vector_data = EmbeddingStore::read_binary("./embeddings").await.unwrap();

            let search_query =
                SearchQuery::new(5, embeddings.data[0].embedding.clone(), Metrics::Cosine);

            let result = search_query.search(&vector_data);

            println!("\nTop {} similar chunks:", search_query.top_k);
            for (i, item) in result.iter().enumerate() {
                println!("\nResult {}:", i + 1);
                println!("Chunk: {}", item.chunk);
                println!("Score: {:.4}", item.score);
            }

            let duration = start.elapsed();
            println!(
                "Search took: {:?} for {} vectors",
                duration, vector_data.total_vectors
            );
        }
        Err(e) => {
            eprintln!("Error fetching embeddings: {}", e);
        }
    }
}
