// These benchmarks are very experimental, Im just doing whatever the hell, I want, Idk how to actually bench vector db by Industry Standards :)
// The bench takes about 20mins+ to run, I think to run separate bench concurrently, and Mutex all the jazz
use blaze_db::core::HNSW;
use blaze_db::core::Metrics;
use blaze_db::prelude::EmbeddingStore;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{File, write};
use std::path::PathBuf;
use std::time::Instant;

#[inline]
fn get_datasets_dir() -> PathBuf {
    PathBuf::from("./datasets")
}

/// Load vectors - binary format
/// [num_vectors: u32][dim: u32][vectors: f32...]
fn load_vectors_mmap() -> (usize, usize, Mmap) {
    let datasets_dir = get_datasets_dir();
    let bin_path = datasets_dir.join("bench_vectors_100k.bin");
    let file = File::open(&bin_path).expect("Failed to open binary file");
    let mmap = unsafe { Mmap::map(&file).expect("Failed to mmap file") };

    // Read header
    let num_vectors = u32::from_le_bytes([mmap[0], mmap[1], mmap[2], mmap[3]]) as usize;
    let dim = u32::from_le_bytes([mmap[4], mmap[5], mmap[6], mmap[7]]) as usize;

    println!(
        "Loaded {} vectors of {} dimensions (Mmapped)",
        num_vectors, dim
    );

    (num_vectors, dim, mmap)
}

#[inline]
/// Get a vector slice from mmap data
fn get_vector(mmap: &Mmap, idx: usize, dim: usize) -> &[f32] {
    let offset = 8 + idx * dim * 4; // 8 bytes header
    unsafe {
        let ptr = mmap.as_ptr().add(offset) as *const f32;
        std::slice::from_raw_parts(ptr, dim)
    }
}

#[inline]
/// Calculate Recall@K - what fraction of HNSW results are in the true top-k (brute force)
fn compare_recall_at_k(
    hnsw_results: &[(String, f32)],
    brute_results: &[(String, f32)],
    k: usize,
) -> f32 {
    let brute_set: HashSet<String> = brute_results
        .iter()
        .take(k)
        .map(|(id, _)| id.clone())
        .collect();
    let mut hits = 0;
    for (id, _) in hnsw_results.iter().take(k) {
        if brute_set.contains(id) {
            hits += 1;
        }
    }
    hits as f32 / k as f32
}

#[derive(Serialize, Deserialize)]
struct BenchmarkResults {
    name: String,
    time_ms: f64,
    metric: String,
}

#[inline]
fn print_results(results: &[BenchmarkResults]) {
    println!("{}", "-".repeat(80));
    println!(
        "{:<30} | {:>15} | {:>20}",
        "Benchmark", "Time (ms)", "Metric"
    );
    println!("{}", "-".repeat(80));

    for r in results {
        if r.time_ms > 0.0 {
            println!("{:<30} | {:>15.2} | {:>20}", r.name, r.time_ms, r.metric);
        } else {
            println!("{:<30} | {:>15} | {:>20}", r.name, "-", r.metric);
        }
    }
    println!("{}", "-".repeat(80));
}

#[inline]
fn save_bench_result(results: &[BenchmarkResults]) {
    let json = serde_json::to_value(results).unwrap();
    write(
        "benches/benchmark_results.json",
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
    println!("Results saved to benches/benchmark_results.json");
}

#[tokio::main]
async fn main() {
    println!("HNSW Benchmark Suite");

    // Load vectors with mmap
    let (num_vectors, dim, mmap) = load_vectors_mmap();
    //let max_vectors = num_vectors.min(100_000);

    let hnsw: HNSW =
        if PathBuf::from(format!("benches/cache/hnsw_index_{}.bin", num_vectors)).exists() {
            println!("Found cached index");

            let store = EmbeddingStore::load_index_file(&PathBuf::from(format!(
                "benches/cache/hnsw_index_{}.bin",
                num_vectors
            )))
            .await;
            store.unwrap().hnsw_store
        } else {
            println!("No cached index found. Building and caching full index...");
            cache_index(num_vectors, dim, &mmap).await.hnsw_store
        };

    let mut results: Vec<BenchmarkResults> = Vec::new();

    println!("[BENCHING]");

    // Varying index construction sizes
    {
        let test_index_sizes = vec![2560, 5120, 10240, 20480, 40960, 81920];

        for size in &test_index_sizes {
            println!("Building index with {} vectors...", size);

            let start = Instant::now();
            let mut hnsw = HNSW::new(32, 256, 18, 1.0 / 16.0_f32.ln(), &Some(Metrics::Cosine));

            for i in 0..*size {
                let vec = get_vector(&mmap, i, dim);
                let level = hnsw.get_random_level();
                let id = format!("chunk_{}", i);
                hnsw.insert(id, vec, format!("metadata_{}", i), level).ok();
            }

            let elapsed = start.elapsed();
            let time_ms = elapsed.as_secs_f64() * 1000.0;
            let vectors_per_sec = *size as f64 / elapsed.as_secs_f64();

            println!(
                "  {} vectors: {:.2} ms ({:.0} vectors/sec)",
                size, time_ms, vectors_per_sec
            );

            // TODO: Do a Search Index here, it will be cool, they will get a nice log graph ;)

            results.push(BenchmarkResults {
                name: format!("construction_{}", size),
                time_ms,
                metric: format!("{:.0} vec/sec", vectors_per_sec),
            });
        }
    }

    // Varying M values (max connections per layer)
    {
        let test_index_size = 10000;
        let test_m_values = vec![18, 32, 48, 64, 96, 128];

        for m in &test_m_values {
            println!("Building index with M={}...", m);

            let start = Instant::now();
            let mut hnsw = HNSW::new(*m, 256, 18, 1.0 / 16.0_f32.ln(), &Some(Metrics::Cosine));

            for i in 0..test_index_size {
                let vec = get_vector(&mmap, i, dim);
                let level = hnsw.get_random_level();
                let id = format!("chunk_{}", i);
                hnsw.insert(id, vec, format!("metadata_{}", i), level).ok();
            }

            let elapsed = start.elapsed();
            let time_ms = elapsed.as_secs_f64() * 1000.0;
            let vectors_per_sec = test_index_size as f64 / elapsed.as_secs_f64();

            println!(
                "  M {}: {:.2} ms ({:.0} vectors/sec)",
                m, time_ms, vectors_per_sec
            );

            results.push(BenchmarkResults {
                name: format!("construction_M_{}", m),
                time_ms,
                metric: format!("{:.0} vec/sec", vectors_per_sec),
            });
        }
    }

    // Varying ef_construction values
    {
        let test_index_size = 10000;
        let test_ef_construction = vec![64, 96, 128, 144, 256, 512, 768, 1024];

        for ef in &test_ef_construction {
            println!("Building index with ef_construction={}...", ef);

            let start = Instant::now();
            let mut hnsw = HNSW::new(18, *ef, 18, 1.0 / 16.0_f32.ln(), &Some(Metrics::Cosine));

            for i in 0..test_index_size {
                let vec = get_vector(&mmap, i, dim);
                let level = hnsw.get_random_level();
                let id = format!("chunk_{}", i);
                hnsw.insert(id, vec, format!("metadata_{}", i), level).ok();
            }

            let elapsed = start.elapsed();
            let time_ms = elapsed.as_secs_f64() * 1000.0;
            let vectors_per_sec = test_index_size as f64 / elapsed.as_secs_f64();

            println!(
                "  ef_construction {}: {:.2} ms ({:.0} vectors/sec)",
                ef, time_ms, vectors_per_sec
            );

            results.push(BenchmarkResults {
                name: format!("construction_ef_{}", ef),
                time_ms,
                metric: format!("{:.0} vec/sec", vectors_per_sec),
            });
        }
    }

    println!("\n[NOTE] All Benches below using Full index");

    {
        let query_count = 5120;
        let k_values = vec![12, 24, 48, 96, 192, 384];

        println!(
            "Running {} search queries with varying k: {:?}",
            query_count, k_values
        );

        for k in k_values {
            let search_start = Instant::now();
            for _i in 0..query_count {
                let query_idx = fastrand::usize(0..num_vectors - 1);
                let query = get_vector(&mmap, query_idx, dim);
                let _ = hnsw.search(query, k, None);
            }
            let search_elapsed = search_start.elapsed();
            let qps = query_count as f64 / search_elapsed.as_secs_f64();

            println!("  QPS: {:.0} queries/sec", qps);

            results.push(BenchmarkResults {
                name: format!("search_qps_at_k_{}", k),
                time_ms: search_elapsed.as_secs_f64() * 1000.0,
                metric: format!("{:.0} qps", qps),
            });
        }
    }

    {
        let latency_queries = 5120; // The more, the better
        let mut latencies: Vec<f64> = Vec::new();

        let k_values = vec![12, 24, 48, 96, 192, 384];

        println!(
            "Measuring search latency for {} queries with varying k: {:?}",
            latency_queries, k_values
        );

        for k in k_values {
            for _i in 0..latency_queries {
                let query_idx = fastrand::usize(0..num_vectors - 1);
                let query = get_vector(&mmap, query_idx, dim);
                let start = Instant::now();
                let _ = hnsw.search(query, k, None);
                latencies.push(start.elapsed().as_secs_f64() * 1000.0);
            }

            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let p50 = latencies[latencies.len() / 2];
            let p95 = latencies[(latencies.len() as f32 * 0.95) as usize];
            let p99 = latencies[(latencies.len() as f32 * 0.99) as usize];
            let avg: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;

            println!(
                "  Latency - Avg: {:.3}ms, p50: {:.3}ms, p95: {:.3}ms, p99: {:.3}ms",
                avg, p50, p95, p99
            );

            results.push(BenchmarkResults {
                name: format!("search_latency_avg_k_{}", k),
                time_ms: avg,
                metric: "avg ms".to_string(),
            });
            results.push(BenchmarkResults {
                name: format!("search_latency_p50_k_{}", k),
                time_ms: p50,
                metric: "p50 ms".to_string(),
            });
            results.push(BenchmarkResults {
                name: format!("search_latency_p95_k_{}", k),
                time_ms: p95,
                metric: "p95 ms".to_string(),
            });
            results.push(BenchmarkResults {
                name: format!("search_latency_p99_k_{}", k),
                time_ms: p99,
                metric: "p99 ms".to_string(),
            });
        }
    }

    // Recall with varying ef_search
    {
        let k = 64;
        let recall_samples = 1280; // The more, the better

        let ef_search_values = vec![32, 64, 128, 256, 512, 768];
        println!(
            "Measuring Recall@{} over {} samples with varying ef: {:?}",
            k, recall_samples, ef_search_values
        );

        for ef in ef_search_values {
            let mut total_recall = 0.0f32;

            for _i in 0..recall_samples {
                let query_idx = fastrand::usize(0..=num_vectors - 1);
                let query = get_vector(&mmap, query_idx, dim);

                // Bypasses adaptive ef_search loop
                let internal_results = hnsw.search_internal(query, k, ef);
                let hnsw_results: Vec<(String, f32)> = internal_results
                    .into_iter()
                    .map(|(id, sim)| (hnsw.nodes[id].node_id.clone(), sim))
                    .collect();

                // Get brute force results
                let bf_results = hnsw.brute_force_search(query, k);

                let recall = compare_recall_at_k(&hnsw_results, &bf_results, k);
                total_recall += recall;
            }

            let avg_recall = total_recall / recall_samples as f32;
            println!("  Recall@{}: {:.2}%", k, avg_recall * 100.0);

            results.push(BenchmarkResults {
                name: format!("recall_at_{}_ef_{}", k, ef),
                time_ms: 0.0,
                metric: format!("{:.2}%", avg_recall * 100.0),
            });
        }
    }

    print_results(&results);
    save_bench_result(&results);
}

async fn cache_index(num_vectors: usize, dim: usize, mmap: &Mmap) -> EmbeddingStore {
    println!("Building index with {} vectors...", num_vectors);

    let mut hnsw = HNSW::new(32, 256, 18, 1.0 / 16.0_f32.ln(), &Some(Metrics::Cosine));

    for i in 0..num_vectors {
        if i % 10000 == 0 {
            println!("  Inserted {}/{}", i, num_vectors,);
        }
        let vec = get_vector(mmap, i, dim);
        let level = hnsw.get_random_level();
        let id = format!("chunk_{}", i);
        hnsw.insert(id, vec, format!("metadata_{}", i), level).ok();
    }

    // Cache the index
    let mut index_store = EmbeddingStore::new(hnsw.clone());
    index_store
        .write_to_disk(&PathBuf::from(format!(
            "benches/cache/hnsw_index_{}.bin",
            num_vectors
        )))
        .await
        .unwrap();

    println!("Index built and cached to disk.");

    index_store
}
