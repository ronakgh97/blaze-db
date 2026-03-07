#[allow(unused)]
use blaze_db::prelude::{EmbeddingStore, VectorData};
use rand::RngExt;
use std::path::PathBuf;

/// Generates a random vector of given dimension with values in range [-1.0, 1.0]
/// Still bad for cosine similarity, but okay for demo purposes
#[inline]
/// Helper to generate random vectors for testing
pub fn generate_random_vectors(num_vectors: usize, dimensions: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::rng();
    (0..num_vectors)
        .map(|_| {
            (0..dimensions)
                .map(|_| rng.random_range(-1.0..1.0))
                .collect()
        })
        .collect()
}

#[allow(unused)]
pub async fn load_index_from_example() -> anyhow::Result<EmbeddingStore> {
    let index_path = PathBuf::from("/amazon_index/HNSW.bin");
    let index = EmbeddingStore::load_index_file(&index_path).await;

    match index {
        Ok(index) => Ok(index),
        Err(e) => {
            eprintln!(
                "Failed to load index from {}.\n Error: {}",
                index_path.display(),
                e
            );
            anyhow::bail!("Failed to load index");
        }
    }
}
