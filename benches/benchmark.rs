// use blaze_db::prelude::{EmbeddingStore, Metrics, VectorData};
// use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
// use std::hint::black_box;
// use tokio::runtime::Runtime;

// fn generate_random_vectors(num_vectors: usize, dimensions: usize) -> VectorData {
//     let mut embeddings = Vec::with_capacity(num_vectors);
//     let mut chunks = Vec::with_capacity(num_vectors);
//
//     for i in 0..num_vectors {
//         let vec: Vec<f32> = (0..dimensions)
//             .map(|j| ((i * dimensions + j) as f32 * 0.001).sin())
//             .collect();
//         embeddings.push(vec);
//         chunks.push(format!("chunk_{}", i));
//     }
//
//     VectorData {
//         chunk: chunks,
//         embedding: embeddings,
//         dimensions,
//     }
// }
//

// fn generate_query_vector(dimensions: usize) -> Vec<f32> {
//     (0..dimensions).map(|i| (i as f32 * 0.002).cos()).collect()
// }

fn main() {
    // Placeholder main function
}

// Search Benchmarks
// fn bench_search_varying_vectors(c: &mut Criterion) {
//     let vector_counts = [1_000, 10_000, 50_000, 100_000];
//     let dimensions = 384; // Common embedding dimension
//
//     let mut group = c.benchmark_group("search_by_vector_count");
//     group.sample_size(30);
//
//     for count in vector_counts {
//         let data = generate_random_vectors(count, dimensions);
//         let query = generate_query_vector(dimensions);
//         let search = SearchQuery::new(10, query, Metrics::Cosine);
//
//         group.throughput(Throughput::Elements(count as u64));
//         group.bench_with_input(BenchmarkId::from_parameter(count), &count, |bench, _| {
//             bench.iter(|| search.search_vector(black_box(&data)))
//         });
//     }
//
//     group.finish();
// }

// fn bench_search_varying_dimensions(c: &mut Criterion) {
//     let dimensions = [128, 384, 768, 1536];
//     let vector_count = 10_000;
//
//     let mut group = c.benchmark_group("search_by_dimensions");
//     group.sample_size(50);
//
//     for dim in dimensions {
//         let data = generate_random_vectors(vector_count, dim);
//         let query = generate_query_vector(dim);
//         let search = SearchQuery::new(10, query, Metrics::Cosine);
//
//         group.throughput(Throughput::Elements((vector_count * dim) as u64));
//         group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |bench, _| {
//             bench.iter(|| search.search_vector(black_box(&data)))
//         });
//     }
//
//     group.finish();
// }

// fn bench_search_varying_top_k(c: &mut Criterion) {
//     let top_k_values = [1, 5, 10, 50, 100];
//     let dimensions = 384;
//     let vector_count = 50_000;
//
//     let data = generate_random_vectors(vector_count, dimensions);
//     let query = generate_query_vector(dimensions);
//
//     let mut group = c.benchmark_group("search_by_top_k");
//     group.sample_size(30);
//
//     for top_k in top_k_values {
//         let search = SearchQuery::new(top_k, query.clone(), Metrics::Cosine);
//
//         group.bench_with_input(BenchmarkId::from_parameter(top_k), &top_k, |bench, _| {
//             bench.iter(|| search.search_vector(black_box(&data)))
//         });
//     }
//
//     group.finish();
// }

// fn bench_all_metrics(c: &mut Criterion) {
//     let dimensions = 384;
//     let vector_count = 10_000;
//
//     let data = generate_random_vectors(vector_count, dimensions);
//     let query = generate_query_vector(dimensions);
//
//     let mut group = c.benchmark_group("search_metrics_comparison");
//
//     for (name, metric) in [
//         ("cosine", Metrics::Cosine),
//         ("euclidean", Metrics::Euclidean),
//         ("dot_product", Metrics::DotProduct),
//     ] {
//         let search = SearchQuery::new(10, query.clone(), metric);
//
//         group.bench_with_input(BenchmarkId::from_parameter(name), &name, |bench, _| {
//             bench.iter(|| search.search_vector(black_box(&data)))
//         });
//     }
//
//     group.finish();
// }

// I/O Benchmarks
// fn bench_load_embeddings(c: &mut Criterion) {
//     let rt = Runtime::new().unwrap();
//
//     // Only run if the embeddings directory exists
//     if !std::path::Path::new("./embeddings").exists() {
//         eprintln!("Skipping load benchmark: ./embeddings directory not found");
//         return;
//     }
//
//     let mut group = c.benchmark_group("io_operations");
//     group.sample_size(20);
//
//     group.bench_function("load_embeddings_from_disk", |bench| {
//         bench.iter(|| {
//             rt.block_on(async { EmbeddingStore::load_binaries(black_box("./embeddings")).await })
//         })
//     });
//
//     group.finish();
// }

// criterion_group!(
//     search_benches,
//     bench_search_varying_vectors,
//     bench_search_varying_dimensions,
//     bench_search_varying_top_k,
//     bench_all_metrics
// );

// criterion_group!(io_benches, bench_load_embeddings);
//
// criterion_main!(search_benches, io_benches);
