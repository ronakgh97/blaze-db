use bincode::{Decode, Encode};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::collections::BinaryHeap;
use wide::f32x8;

#[allow(unused)]
#[deprecated(since = "2026-01-08", note = "Use `HNSW::new` instead")]
#[derive(Serialize, Deserialize)]
pub struct SearchQuery {
    pub top_k: usize,
    pub query_vector: Vec<f32>,
    pub metric: Metrics,
}

#[allow(unused)]
#[deprecated(since = "2026-01-08", note = "Use `HNSW::new` instead")]
#[derive(Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: String,
    pub score: f32,
}

// impl SearchQuery {
//     pub fn new(top_k: usize, query_vector: Vec<f32>, metric: Metrics) -> Self {
//         Self {
//             top_k,
//             query_vector,
//             metric,
//         }
//     }
//
//     pub fn search_vector(&self, data: &VectorData) -> Vec<SearchResult> {
//         let mut results: Vec<SearchResult> = data
//             .embedding
//             .par_iter()
//             .enumerate()
//             .map(|(idx, vector)| {
//                 let score = self.metric.calculate(&self.query_vector, vector);
//                 SearchResult {
//                     chunk: data.chunk[idx].to_string(),
//                     score,
//                 }
//             })
//             .collect();
//
//         // Sort results by score in descending order
//         results.sort_by(|a, b| {
//             // Compare scores, treating NaN as less than any number
//             match a.score.is_nan().cmp(&b.score.is_nan()) {
//                 Ordering::Equal => b.score.partial_cmp(&a.score).unwrap(),
//                 other => other,
//             }
//         });
//
//         // Return top_k results
//         results.into_iter().take(self.top_k).collect()
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode, PartialEq, ValueEnum)]
pub enum Metrics {
    Cosine,
    Euclidean,
    DotProduct,
}

impl Metrics {
    pub fn calculate(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Metrics::Cosine => cosine_similarity(a, b),
            Metrics::Euclidean => euclidean_similarity(a, b),
            Metrics::DotProduct => dot_product(a, b),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Metrics::Cosine => "COSINE".to_string(),
            Metrics::Euclidean => "EUCLIDEAN".to_string(),
            Metrics::DotProduct => "DOT_PRODUCT".to_string(),
        }
    }
}

#[inline]
/// SIMD-optimized cosine similarity using 8-wide f32 vectors
/// Returns value in [-1, 1]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

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

#[inline]
/// SIMD-optimized Euclidean similarity
/// Returns value in (0, 1]
pub fn euclidean_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let chunks = a.len() / 8;
    let mut sum_sq = f32x8::ZERO;

    for i in 0..chunks {
        let offset = i * 8;
        let va = f32x8::from(&a[offset..offset + 8]);
        let vb = f32x8::from(&b[offset..offset + 8]);
        let diff = va - vb;
        sum_sq += diff * diff;
    }

    let arr = sum_sq.to_array();
    let mut distance_sq: f32 = arr.iter().sum();

    // Handle remainder
    let remainder_start = chunks * 8;
    for i in remainder_start..a.len() {
        let diff = a[i] - b[i];
        distance_sq += diff * diff;
    }

    1.0 / (1.0 + distance_sq.sqrt())
}

#[inline]
/// SIMD-optimized raw dot product
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let chunks = a.len() / 8;
    let mut sum = f32x8::ZERO;

    for i in 0..chunks {
        let offset = i * 8;
        let va = f32x8::from(&a[offset..offset + 8]);
        let vb = f32x8::from(&b[offset..offset + 8]);
        sum += va * vb;
    }

    let arr = sum.to_array();
    let mut total: f32 = arr.iter().sum();

    // Handle remainder
    let remainder_start = chunks * 8;
    for i in remainder_start..a.len() {
        total += a[i] * b[i];
    }

    total
}
