use blaze_db::core::hnsw::HNSW;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rand::Rng;
use std::hint::black_box;

#[allow(unused)]
/// Generate random vectors for benchmarking
fn generate_random_vectors(num_vectors: usize, dimensions: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::rng();
    (0..num_vectors)
        .map(|_| {
            (0..dimensions)
                .map(|_| rng.random_range(-1.0..1.0))
                .collect()
        })
        .collect()
}

/// Generate deterministic vectors for reproducible benchmarks
fn generate_deterministic_vectors(num_vectors: usize, dimensions: usize) -> Vec<Vec<f32>> {
    (0..num_vectors)
        .map(|i| {
            (0..dimensions)
                .map(|j| ((i * dimensions + j) as f32 * 0.001).sin())
                .collect()
        })
        .collect()
}

/// Generate a single query vector
fn generate_query_vector(dimensions: usize) -> Vec<f32> {
    (0..dimensions).map(|i| (i as f32 * 0.002).cos()).collect()
}

/// Benchmark HNSW construction with varying number of vectors (100, 500, 1K, 5K) 384-dim
fn bench_hnsw_construction_varying_vectors(c: &mut Criterion) {
    let vector_counts = [1_000, 5_000, 10_000, 50_000];
    let dimensions = 384;

    let mut group = c.benchmark_group("hnsw_construction_by_vector_count");
    group.sample_size(10); // Reduce sample size for slower operations

    for count in vector_counts {
        let vectors = generate_deterministic_vectors(count, dimensions);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |bench, _| {
            bench.iter(|| {
                let mut hnsw = HNSW::new(16, 200, 5, 1.0 / 16.0_f32.ln());
                for (i, vec) in vectors.iter().enumerate() {
                    let level = hnsw.get_random_level();
                    hnsw.insert(black_box(vec), format!("chunk_{}", i), level);
                }
                hnsw
            })
        });
    }

    group.finish();
}

/// Benchmark HNSW construction with varying dimensions (384, 768, 1024, 1536) with 1000 vectors
fn bench_hnsw_construction_varying_dimensions(c: &mut Criterion) {
    let dimensions = [384, 768, 1024, 1536];
    let vector_count = 1_000;

    let mut group = c.benchmark_group("hnsw_construction_by_dimensions");
    group.sample_size(10);

    for dim in dimensions {
        let vectors = generate_deterministic_vectors(vector_count, dim);

        group.throughput(Throughput::Elements((vector_count * dim) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |bench, _| {
            bench.iter(|| {
                let mut hnsw = HNSW::new(16, 200, 5, 1.0 / 16.0_f32.ln());
                for (i, vec) in vectors.iter().enumerate() {
                    let level = hnsw.get_random_level();
                    hnsw.insert(black_box(vec), format!("chunk_{}", i), level);
                }
                hnsw
            })
        });
    }

    group.finish();
}

/// Benchmark HNSW construction with different max_neighbors (M) values (8, 16, 32, 48) with 1000 vectors, 384-dim
fn bench_hnsw_construction_varying_m(c: &mut Criterion) {
    let m_values = [8, 16, 32, 48];
    let vector_count = 1_000;
    let dimensions = 384;

    let vectors = generate_deterministic_vectors(vector_count, dimensions);

    let mut group = c.benchmark_group("hnsw_construction_by_max_neighbors");
    group.sample_size(10);

    for m in m_values {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |bench, &m| {
            bench.iter(|| {
                let mut hnsw = HNSW::new(m, 200, 5, 1.0 / (m as f32).ln());
                for (i, vec) in vectors.iter().enumerate() {
                    let level = hnsw.get_random_level();
                    hnsw.insert(black_box(vec), format!("chunk_{}", i), level);
                }
                hnsw
            })
        });
    }

    group.finish();
}

/// Benchmark HNSW construction with different ef_construction values (50, 100, 200, 400) with 1000 vectors, 384-dim
fn bench_hnsw_construction_varying_ef(c: &mut Criterion) {
    let ef_values = [50, 100, 200, 400];
    let vector_count = 1_000;
    let dimensions = 384;

    let vectors = generate_deterministic_vectors(vector_count, dimensions);

    let mut group = c.benchmark_group("hnsw_construction_by_ef_construction");
    group.sample_size(10);

    for ef in ef_values {
        group.bench_with_input(BenchmarkId::from_parameter(ef), &ef, |bench, &ef| {
            bench.iter(|| {
                let mut hnsw = HNSW::new(16, ef, 5, 1.0 / 16.0_f32.ln());
                for (i, vec) in vectors.iter().enumerate() {
                    let level = hnsw.get_random_level();
                    hnsw.insert(black_box(vec), format!("chunk_{}", i), level);
                }
                hnsw
            })
        });
    }

    group.finish();
}

#[inline]
/// Helper to build a pre-populated HNSW index with params: mx_n: 16, ef_c: 200, mx_l 5, e: ln(1/16)
fn build_hnsw_index(num_vectors: usize, dimensions: usize) -> HNSW {
    let mut hnsw = HNSW::new(16, 200, 5, 1.0 / 16.0_f32.ln());
    let vectors = generate_deterministic_vectors(num_vectors, dimensions);

    for (i, vec) in vectors.iter().enumerate() {
        let level = hnsw.get_random_level();
        hnsw.insert(&vec, format!("chunk_{}", i), level);
    }

    hnsw
}

/// Benchmark search with varying index sizes
fn bench_hnsw_search_varying_index_size(c: &mut Criterion) {
    let index_sizes = [1_000, 5_000, 10_000, 50_000];
    let dimensions = 384;
    let k = 10;

    let mut group = c.benchmark_group("hnsw_search_by_index_size");
    group.sample_size(30);

    for size in index_sizes {
        let hnsw = build_hnsw_index(size, dimensions);
        let query = generate_query_vector(dimensions);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, _| {
            bench.iter(|| hnsw.search(black_box(&query), black_box(k)))
        });
    }

    group.finish();
}

/// Benchmark search with varying k (top-k values) 1,5,10,20,50,100 on 10K index, 384-dim
fn bench_hnsw_search_varying_k(c: &mut Criterion) {
    let k_values = [1, 5, 10, 20, 50, 100];
    let index_size = 10_000;
    let dimensions = 384;

    let hnsw = build_hnsw_index(index_size, dimensions);
    let query = generate_query_vector(dimensions);

    let mut group = c.benchmark_group("hnsw_search_by_top_k");
    group.sample_size(50);

    for k in k_values {
        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |bench, &k| {
            bench.iter(|| hnsw.search(black_box(&query), black_box(k)))
        });
    }

    group.finish();
}

/// Benchmark search with varying dimensions (64, 128, 256, 384, 512) on 10K index
fn bench_hnsw_search_varying_dimensions(c: &mut Criterion) {
    let dimensions = [64, 128, 256, 384, 512];
    let index_size = 10_000;
    let k = 10;

    let mut group = c.benchmark_group("hnsw_search_by_dimensions");
    group.sample_size(30);

    for dim in dimensions {
        let hnsw = build_hnsw_index(index_size, dim);
        let query = generate_query_vector(dim);

        group.throughput(Throughput::Elements((index_size * dim) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |bench, _| {
            bench.iter(|| hnsw.search(black_box(&query), black_box(k)))
        });
    }

    group.finish();
}

/// Benchmark search with metadata retrieval
fn bench_hnsw_search_with_metadata(c: &mut Criterion) {
    let index_size = 10_000;
    let dimensions = 384;
    let k = 10;

    let hnsw = build_hnsw_index(index_size, dimensions);
    let query = generate_query_vector(dimensions);

    let mut group = c.benchmark_group("hnsw_search_metadata");
    group.sample_size(50);

    group.bench_function("search_plain", |bench| {
        bench.iter(|| hnsw.search(black_box(&query), black_box(k)))
    });

    group.bench_function("search_with_metadata", |bench| {
        bench.iter(|| hnsw.search_with_metadata(black_box(&query), black_box(k)))
    });

    group.finish();
}

/// Benchmark typical embedding dimensions (OpenAI, sentence-transformers, etc.)
fn bench_hnsw_common_embedding_dimensions(c: &mut Criterion) {
    let configs = [
        ("openai_ada_002", 1536),     // OpenAI text-embedding-ada-002
        ("sentence_bert_base", 768),  // BERT-base sentence embeddings
        ("sentence_bert_small", 384), // MiniLM sentence embeddings
        ("cohere_embed", 1024),       // Cohere embeddings
    ];
    let index_size = 10_000;
    let k = 10;

    let mut group = c.benchmark_group("hnsw_common_embeddings");
    group.sample_size(20);

    for (name, dim) in configs {
        let hnsw = build_hnsw_index(index_size, dim);
        let query = generate_query_vector(dim);

        group.bench_with_input(BenchmarkId::new("search", name), &dim, |bench, _| {
            bench.iter(|| hnsw.search(black_box(&query), black_box(k)))
        });
    }

    group.finish();
}

/// Benchmark batch query scenario (10, 50, 100, 500 queries) on 10K index, 384-dim
fn bench_hnsw_batch_queries(c: &mut Criterion) {
    let batch_sizes = [10, 50, 100, 500];
    let index_size = 10_000;
    let dimensions = 384;
    let k = 10;

    let hnsw = build_hnsw_index(index_size, dimensions);

    let mut group = c.benchmark_group("hnsw_batch_queries");
    group.sample_size(20);

    for batch_size in batch_sizes {
        let queries: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| {
                (0..dimensions)
                    .map(|j| ((i * dimensions + j) as f32 * 0.003).cos())
                    .collect()
            })
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |bench, _| {
                bench.iter(|| {
                    for query in &queries {
                        black_box(hnsw.search(black_box(query), black_box(k)));
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark incremental inserts into existing index (500, 1K, 5K) adding 100 vectors each time, 384-dim
fn bench_hnsw_incremental_insert(c: &mut Criterion) {
    let initial_sizes = [500, 1_000, 5_000];
    let dimensions = 384;
    let inserts_per_bench = 100;

    let mut group = c.benchmark_group("hnsw_incremental_insert");
    group.sample_size(20);

    for initial_size in initial_sizes {
        let new_vectors = generate_deterministic_vectors(inserts_per_bench, dimensions);

        group.throughput(Throughput::Elements(inserts_per_bench as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(initial_size),
            &initial_size,
            |bench, &size| {
                bench.iter(|| {
                    let mut hnsw = build_hnsw_index(size, dimensions);
                    for (i, vec) in new_vectors.iter().enumerate() {
                        let level = hnsw.get_random_level();
                        hnsw.insert(black_box(vec), format!("new_chunk_{}", i), black_box(level));
                    }
                    hnsw
                })
            },
        );
    }

    group.finish();
}

/// Benchmark single insert operation at different index sizes (100, 500, 1K, 5K) 384-dim
fn bench_hnsw_single_insert_at_scale(c: &mut Criterion) {
    let index_sizes = [100, 500, 1_000, 5_000];
    let dimensions = 384;

    let mut group = c.benchmark_group("hnsw_single_insert_scalability");
    group.sample_size(30);

    for size in index_sizes {
        let new_vector = generate_query_vector(dimensions);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bench, &size| {
            bench.iter_batched(
                || build_hnsw_index(size, dimensions),
                |mut hnsw| {
                    let level = hnsw.get_random_level();
                    hnsw.insert(black_box(&new_vector), "new_chunk".to_string(), level);
                    hnsw
                },
                criterion::BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

criterion_group!(
    construction_benches,
    bench_hnsw_construction_varying_vectors,
    bench_hnsw_construction_varying_dimensions,
    bench_hnsw_construction_varying_m,
    bench_hnsw_construction_varying_ef,
);

criterion_group!(
    search_benches,
    bench_hnsw_search_varying_index_size,
    bench_hnsw_search_varying_k,
    bench_hnsw_search_varying_dimensions,
    bench_hnsw_search_with_metadata,
);

criterion_group!(
    realworld_benches,
    bench_hnsw_common_embedding_dimensions,
    bench_hnsw_batch_queries,
);

criterion_group!(
    incremental_benches,
    bench_hnsw_incremental_insert,
    bench_hnsw_single_insert_at_scale,
);

criterion_main!(
    construction_benches,
    search_benches,
    realworld_benches,
    incremental_benches,
);
