#[allow(unused)]
use blaze_db::prelude::{EmbeddingStore, VectorData};
use rand::RngExt;
use std::path::PathBuf;
use wide::f32x8;

/// Generates a random vector of given dimension with values in range [-2.0, 2.0]
/// Still bad for cosine similarity, but okay for demo purposes
#[inline]
pub fn generate_random_vector(dimension: usize) -> Vec<f32> {
    let mut rng = rand::rng();

    let mut vector = vec![0.0f32; dimension];
    for i in 0..dimension {
        vector[i] = rng.random_range(-2.0..2.0);
    }
    vector
}

/// Cosine similarity using 8-wide f32 vectors
/// Higher the value, more similar the vectors are
/// Cos theta decrease as angle increases from 0 to pi, so does vector similarity as they diverge
/// Returns value in [-1, 1]
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "What?! Vectors must be the same length, Imma panic!"
    );

    let chunks = a.len() / 8;
    let mut dot = f32x8::ZERO;
    let mut norm_a = f32x8::ZERO;
    let mut norm_b = f32x8::ZERO;

    // Process 8 elements at a time with SIMD
    for i in 0..chunks {
        let offset = i * 8;
        let va = f32x8::from(&a[offset..offset + 8]);
        let vb = f32x8::from(&b[offset..offset + 8]);
        dot += va * vb;
        norm_a += va * va;
        norm_b += vb * vb;
    }

    // Reduce SIMD vectors to scalars
    let arr_dot = dot.to_array();
    let arr_na = norm_a.to_array();
    let arr_nb = norm_b.to_array();

    let mut dot_sum: f32 = arr_dot.iter().sum();
    let mut na_sum: f32 = arr_na.iter().sum();
    let mut nb_sum: f32 = arr_nb.iter().sum();

    // Handle remaining elements (tail)
    let remainder_start = chunks * 8;
    for i in remainder_start..a.len() {
        dot_sum += a[i] * b[i];
        na_sum += a[i] * a[i];
        nb_sum += b[i] * b[i];
    }

    let denominator = (na_sum * nb_sum).sqrt();
    if denominator < f32::EPSILON {
        0.0
    } else {
        dot_sum / denominator
    }
}

/// Loads an explicit sample HNSW index from disk for testing/demo purposes
#[inline]
#[allow(unused)]
pub async fn load_sample_hnsw_index() -> EmbeddingStore {
    let path_to_sample_index: PathBuf = PathBuf::from("./embeddings/index_batch_5.bin");

    let hnsw_index = EmbeddingStore::load_binary_file(&path_to_sample_index)
        .await
        .expect("Failed to load embeddings");

    hnsw_index
}
