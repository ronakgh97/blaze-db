use blaze_db::core::HNSW;
use blaze_db::core::Metrics;
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
/// Calculate Recall@K given HNSW results and brute force results
fn recall_at_k(hnsw_results: &[(String, f32)], brute_results: &[(String, f32)], k: usize) -> f32 {
    let hnsw_set: HashSet<_> = hnsw_results
        .iter()
        .take(k)
        .map(|(id, _)| id.clone())
        .collect();
    let mut hits = 0;
    for (id, _) in brute_results.iter().take(k) {
        if hnsw_set.contains(id) {
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
    println!();
}

#[inline]
fn save_json(results: &[BenchmarkResults]) {
    let json = serde_json::json!({
        "benchmarks": results.iter().map(|r| {
            BenchmarkResults {
                name: r.name.clone(),
                time_ms: r.time_ms,
                metric: r.metric.clone(),
                }
        }).collect::<Vec<_>>()
    });

    write(
        "benches/benchmark_results.json",
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
    println!("Results saved to benchmark_results.json");
}

fn main() {
    println!("HNSW Benchmark Suite");

    let mut results: Vec<BenchmarkResults> = Vec::new();

    // Load vectors with mmap
    let (num_vectors, dim, mmap) = load_vectors_mmap();
    let max_vectors = num_vectors.min(75_000);

    let test_sizes = vec![5000, 10000, 20000];

    for size in &test_sizes {
        if *size > max_vectors {
            break;
        }

        println!("Building index with {} vectors...", size);

        let start = Instant::now();
        let mut hnsw = HNSW::new(16, 200, 5, 1.0 / 16.0_f32.ln(), &Some(Metrics::Cosine));

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

        results.push(BenchmarkResults {
            name: format!("construction_{}", size),
            time_ms,
            metric: format!("{:.0} vec/sec", vectors_per_sec),
        });
    }

    println!("\nBUILDING FULL INDEX ({} vectors)", max_vectors);

    let build_start = Instant::now();
    let mut hnsw = HNSW::new(16, 200, 5, 1.0 / 16.0_f32.ln(), &Some(Metrics::Cosine));

    for i in 0..max_vectors {
        if i % 10000 == 0 {
            println!(
                "  Inserted {}/{} ({:.1}%)",
                i,
                max_vectors,
                i as f32 / max_vectors as f32 * 100.0
            );
        }
        let vec = get_vector(&mmap, i, dim);
        let level = hnsw.get_random_level();
        let id = format!("chunk_{}", i);
        hnsw.insert(id, vec, format!("metadata_{}", i), level).ok();
    }

    let build_elapsed = build_start.elapsed();
    let build_time_ms = build_elapsed.as_secs_f64() * 1000.0;
    let build_vectors_per_sec = max_vectors as f64 / build_elapsed.as_secs_f64();

    println!(
        "  Built in {:.2} ms ({:.0} vectors/sec)",
        build_time_ms, build_vectors_per_sec
    );

    results.push(BenchmarkResults {
        name: format!("construction_{}", max_vectors),
        time_ms: build_time_ms,
        metric: format!("{:.0} vec/sec", build_vectors_per_sec),
    });

    let query_count = 1000;
    let k = 10;

    println!("Running {} search queries...", query_count);

    let search_start = Instant::now();
    for i in 0..query_count {
        let query_idx = (i * 7) % max_vectors;
        let query = get_vector(&mmap, query_idx, dim);
        let _ = hnsw.search(query, k, None);
    }
    let search_elapsed = search_start.elapsed();
    let qps = query_count as f64 / search_elapsed.as_secs_f64();

    println!("  QPS: {:.0} queries/sec", qps);

    results.push(BenchmarkResults {
        name: "search_qps".to_string(),
        time_ms: search_elapsed.as_secs_f64() * 1000.0,
        metric: format!("{:.0} qps", qps),
    });

    let latency_queries = 100;
    let mut latencies: Vec<f64> = Vec::new();

    println!(
        "Measuring search latency for {} queries...",
        latency_queries
    );

    for i in 0..latency_queries {
        let query_idx = (i * 13) % max_vectors;
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
        name: "search_latency_avg".to_string(),
        time_ms: avg,
        metric: "avg ms".to_string(),
    });
    results.push(BenchmarkResults {
        name: "search_latency_p50".to_string(),
        time_ms: p50,
        metric: "p50 ms".to_string(),
    });
    results.push(BenchmarkResults {
        name: "search_latency_p95".to_string(),
        time_ms: p95,
        metric: "p95 ms".to_string(),
    });
    results.push(BenchmarkResults {
        name: "search_latency_p99".to_string(),
        time_ms: p99,
        metric: "p99 ms".to_string(),
    });

    let recall_samples = 100;

    let mut total_recall = 0.0f32;

    println!("Measuring Recall@{} over {} samples...", k, recall_samples);

    for i in 0..recall_samples {
        let query_idx = (i * 17) % max_vectors;
        let query = get_vector(&mmap, query_idx, dim);

        // Get HNSW results
        let hnsw_results = hnsw.search(query, k, None);

        // Get brute force results using HNSW::brute_force_search
        let bf_results = hnsw.brute_force_search(query, k);

        let recall = recall_at_k(&hnsw_results, &bf_results, k);
        total_recall += recall;
    }

    let avg_recall = total_recall / recall_samples as f32;
    println!("  Recall@{}: {:.2}%", k, avg_recall * 100.0);

    results.push(BenchmarkResults {
        name: format!("recall_at_{}", k),
        time_ms: 0.0,
        metric: format!("{:.2}%", avg_recall * 100.0),
    });

    let batch_sizes = vec![10, 50, 100];

    for batch_size in batch_sizes {
        let start = Instant::now();
        for i in 0..batch_size {
            let query_idx = (i * 23) % max_vectors;
            let query = get_vector(&mmap, query_idx, dim);
            let _ = hnsw.search(query, k, None);
        }
        let elapsed = start.elapsed();
        let batch_qps = batch_size as f64 / elapsed.as_secs_f64();

        println!("  Batch {}: {:.0} queries/sec", batch_size, batch_qps);

        results.push(BenchmarkResults {
            name: format!("batch_search_{}", batch_size),
            time_ms: elapsed.as_secs_f64() * 1000.0,
            metric: format!("{:.0} qps", batch_qps),
        });
    }

    let k_values = vec![1, 5, 10, 20, 50];

    for k_val in k_values {
        let start = Instant::now();
        for i in 0..100 {
            let query_idx = (i * 11) % max_vectors;
            let query = get_vector(&mmap, query_idx, dim);
            let _ = hnsw.search(query, k_val, None);
        }
        let elapsed = start.elapsed();
        let ms_per_query = elapsed.as_secs_f64() * 1000.0 / 100.0;

        println!("  k={}: {:.2} ms/query", k_val, ms_per_query);

        results.push(BenchmarkResults {
            name: format!("search_k_{}", k_val),
            time_ms: ms_per_query,
            metric: format!("{:.2} ms/query", ms_per_query),
        });
    }

    let ef_values = vec![10, 30, 50, 100, 200];

    for ef in ef_values {
        let start = Instant::now();
        for i in 0..100 {
            let query_idx = (i * 19) % max_vectors;
            let query = get_vector(&mmap, query_idx, dim);
            let _ = hnsw.search(query, k, Some(ef));
        }
        let elapsed = start.elapsed();
        let ms_per_query = elapsed.as_secs_f64() * 1000.0 / 100.0;

        println!("  ef={}: {:.2} ms/query", ef, ms_per_query);

        results.push(BenchmarkResults {
            name: format!("search_ef_{}", ef),
            time_ms: ms_per_query,
            metric: format!("{:.2} ms/query", ms_per_query),
        });
    }

    print_results(&results);
    save_json(&results);
}
