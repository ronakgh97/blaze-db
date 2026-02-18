// Hardcore stress tests for concurrency - Let's break this thing

use blaze_db::prelude::{
    CreateDatabaseRequest, CreateSourceRequest, InsertRequest, VectorDataDto, VectorQueryRequest,
};
use rand::RngExt;
use reqwest::Client;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
#[allow(unused)]
use tokio::sync::{Barrier, Semaphore};
use uuid::Uuid;

const BASE_URL: &str = "http://localhost:8080";
const HEALTH_ENDPOINT: &str = "/v1/blazedb/health";
const CREATE_DB_ENDPOINT: &str = "/v1/blazedb/databases/create";
const CREATE_SOURCE_ENDPOINT: &str = "/v1/blazedb/sources/create";
const INSERT_ENDPOINT: &str = "/v1/blazedb/insert";
const VECTOR_QUERY_ENDPOINT: &str = "/v1/blazedb/query/vector";

fn create_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("Failed to create HTTP client")
}

async fn wait_for_server(max_attempts: u32) -> bool {
    let client = create_client();
    for _ in 1..=max_attempts {
        if let Ok(response) = client
            .get(format!("{}{}", BASE_URL, HEALTH_ENDPOINT))
            .send()
            .await
        {
            if response.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

#[inline]
/// Helper to generate random vectors for testing
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

/// Thundering Herd - 100 concurrent cache misses on same database
/// This tests the per-database loading lock to prevent duplicate loads
#[tokio::test]
#[ignore]
async fn stress_test_thundering_herd_same_database() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();

    // Create source and database
    let source_name = format!("stress_src_{}", timestamp);
    let db_name = format!("stress_db_{}", timestamp);

    let source_req = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_req)
        .send()
        .await
        .unwrap();

    // Insert initial data
    let vectors: Vec<VectorDataDto> = (0..5000)
        .map(|i| VectorDataDto {
            id: Uuid::new_v4().to_string(),
            embedding: generate_random_vectors(1, 1024)[0].clone(),
            metadata: format!("vector_{}", i),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&insert_req)
        .send()
        .await
        .unwrap();

    println!("Setup complete, starting thundering herd test...");

    let num_concurrent = 100;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let success_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    let mut handles = vec![];

    for _i in 0..num_concurrent {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let success_count = Arc::clone(&success_count);

        let handle = tokio::spawn(async move {
            // Wait for all threads to be ready
            barrier.wait().await;

            let query_start = Instant::now();

            let query_req = VectorQueryRequest {
                query_vector: generate_random_vectors(1, 1024)[0].clone(),
                database: db_name,
                source: source_name,
                top_k: 10,
            };

            let response = client
                .post(format!("{}{}", BASE_URL, VECTOR_QUERY_ENDPOINT))
                .json(&query_req)
                .send()
                .await;

            let elapsed = query_start.elapsed();

            if response.is_ok() && response.unwrap().status().is_success() {
                success_count.fetch_add(1, Ordering::SeqCst);
            }

            elapsed
        });

        handles.push(handle);
    }

    let results: Vec<Duration> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    let total_elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);

    // Calculate stats
    let min_latency = results.iter().min().unwrap();
    let max_latency = results.iter().max().unwrap();
    let avg_latency = results.iter().sum::<Duration>() / results.len() as u32;

    println!("\n THUNDERING HERD TEST RESULTS:");
    println!("  Concurrent requests: {}", num_concurrent);
    println!("  Successful: {}/{}", success, num_concurrent);
    println!("  Total time: {:?}", total_elapsed);
    println!("  Min latency: {:?}", min_latency);
    println!("  Max latency: {:?}", max_latency);
    println!("  Avg latency: {:?}", avg_latency);
    println!(
        "- Latency ratio: {:.2}x",
        max_latency.as_millis() as f64 / min_latency.as_millis() as f64
    );

    // With proper per-DB locking, only ONE load should happen
    // All other threads should wait and reuse the cached data
    // We allow 5x spread to account for that, but if we see 100x+ spread, it's a sign of duplicate loads happening
    assert!(
        max_latency.as_millis() < min_latency.as_millis() * 5,
        "Latency spread too high - possible duplicate loads! Min: {:?}, Max: {:?}",
        min_latency,
        max_latency
    );
    assert_eq!(success, num_concurrent as u64, "Some requests failed!");

    println!("No duplicate loads detected - per-database locking works!");
}

/// Concurrent writes to different databases
/// This tests that write locks don't block each other across databases
#[tokio::test]
#[ignore]
async fn stress_test_concurrent_writes_different_databases() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let num_databases = 50;
    let vectors_per_db = 2000;

    // Create source
    let source_name = format!("stress_src_{}", timestamp);
    let source_req = CreateSourceRequest {
        backup_interval_hours: Some(1),
        source_name: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    println!("Source created, creating {} databases...", num_databases);

    // Create databases
    let mut db_names = vec![];
    for i in 0..num_databases {
        let db_name = format!("stress_db_{}_{}", timestamp, i);
        let db_req = CreateDatabaseRequest {
            name: db_name.clone(),
            source: source_name.clone(),
            metrics: None,
            dimensions: 1024,
            backup_interval_hours: None,
        };
        client
            .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
            .json(&db_req)
            .send()
            .await
            .unwrap();
        db_names.push(db_name);
    }

    println!("Databases created, starting concurrent writes...");

    let start = Instant::now();
    let barrier = Arc::new(Barrier::new(num_databases));
    let success_count = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    for (idx, db_name) in db_names.iter().enumerate() {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let success_count = Arc::clone(&success_count);

        let handle = tokio::spawn(async move {
            // Wait for all threads to be ready
            barrier.wait().await;

            let write_start = Instant::now();

            // Generate vectors
            let vectors: Vec<VectorDataDto> = (0..vectors_per_db)
                .map(|i| VectorDataDto {
                    id: Uuid::new_v4().to_string(),
                    embedding: generate_random_vectors(1, 1024)[0].clone(),
                    metadata: format!("db_{}_vector_{}", idx, i),
                })
                .collect();

            let insert_req = InsertRequest {
                nodes: vec![vectors],
                database: db_name,
                source: source_name,
            };

            let response = client
                .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
                .json(&insert_req)
                .send()
                .await;

            let elapsed = write_start.elapsed();

            if response.is_ok() && response.unwrap().status().is_success() {
                success_count.fetch_add(1, Ordering::SeqCst);
            }

            (idx, elapsed)
        });

        handles.push(handle);
    }

    let results: Vec<(usize, Duration)> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    let total_elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);

    // Calculate stats
    let min_write_time = results.iter().map(|(_, d)| d).min().unwrap();
    let max_write_time = results.iter().map(|(_, d)| d).max().unwrap();
    let avg_write_time = results.iter().map(|(_, d)| d).sum::<Duration>() / results.len() as u32;

    println!("\n CONCURRENT WRITES TEST RESULTS:");
    println!("  Databases written: {}", num_databases);
    println!("  Vectors per database: {}", vectors_per_db);
    println!("  Successful: {}/{}", success, num_databases);
    println!("  Total time: {:?}", total_elapsed);
    println!("  Min write time: {:?}", min_write_time);
    println!("  Max write time: {:?}", max_write_time);
    println!("  Avg write time: {:?}", avg_write_time);

    // If writes were truly concurrent, total time should be close to max write time
    // Not sum of all write times (which would be if they were serialized)
    let expected_sequential_time = avg_write_time * num_databases as u32;
    println!("  Expected if sequential: {:?}", expected_sequential_time);
    println!(
        "  Speedup: {:.2}x",
        expected_sequential_time.as_secs_f64() / total_elapsed.as_secs_f64()
    );

    assert_eq!(success, num_databases as u64, "Some writes failed!");
    assert!(
        total_elapsed < expected_sequential_time / 2,
        "Writes appear to be serialized!"
    );

    println!("Concurrent writes to different databases work in parallel!");
}

/// Mixed read/write workload
/// Readers should not block each other, writers should not block readers on different DBs
#[tokio::test]
#[ignore]
async fn stress_test_mixed_read_write_workload() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();

    // Create source and 10 databases
    let source_name = format!("stress_src_{}", timestamp);
    let source_req = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: source_name.clone(),
    };
    client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_req)
        .send()
        .await
        .unwrap();

    let num_databases = 75;
    let num_vectors = 1024;
    let mut db_names = vec![];

    for i in 0..num_databases {
        let db_name = format!("stress_db_{}_{}", timestamp, i);
        let db_req = CreateDatabaseRequest {
            name: db_name.clone(),
            source: source_name.clone(),
            metrics: None,
            dimensions: 1024,
            backup_interval_hours: None,
        };
        client
            .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
            .json(&db_req)
            .send()
            .await
            .unwrap();

        // Insert initial data
        let vectors: Vec<VectorDataDto> = (0..num_vectors)
            .map(|j| VectorDataDto {
                id: Uuid::new_v4().to_string(),
                embedding: generate_random_vectors(1, 1024)[0].clone(),
                metadata: format!("vector_{}", j),
            })
            .collect();

        let insert_req = InsertRequest {
            nodes: vec![vectors],
            database: db_name.clone(),
            source: source_name.clone(),
        };
        client
            .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
            .json(&insert_req)
            .send()
            .await
            .unwrap();

        db_names.push(db_name);
    }

    println!(" Setup complete with {} databases", num_databases);
    println!(" Starting mixed read/write stress test...");

    let num_readers = 50;
    let read_queries = 50;
    let num_writers = 25;
    let write_queries = 10;
    let total_workers = num_readers + num_writers;

    let barrier = Arc::new(Barrier::new(total_workers));
    let read_success = Arc::new(AtomicU64::new(0));
    let write_success = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let mut handles = vec![];

    // Spawn readers
    for i in 0..num_readers {
        let client = create_client();
        let db_names = db_names.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let read_success = Arc::clone(&read_success);

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut successful = 0;
            let mut total_latency = Duration::ZERO;

            for _j in 0..read_queries {
                use fastrand::usize;
                let db_idx = usize(0..db_names.len());
                let query_start = Instant::now();

                let query_req = VectorQueryRequest {
                    query_vector: generate_random_vectors(1, 1024)[0].clone(),
                    database: db_names[db_idx].clone(),
                    source: source_name.clone(),
                    top_k: 5,
                };

                if let Ok(response) = client
                    .post(format!("{}{}", BASE_URL, VECTOR_QUERY_ENDPOINT))
                    .json(&query_req)
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        successful += 1;
                    }
                }

                total_latency += query_start.elapsed();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            read_success.fetch_add(successful, Ordering::SeqCst);
            (i, successful, total_latency)
        });

        handles.push(handle);
    }

    // Spawn writers
    for i in 0..num_writers {
        let client = create_client();
        let db_names = db_names.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let write_success = Arc::clone(&write_success);

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut successful = 0;
            let mut total_latency = Duration::ZERO;

            for j in 0..write_queries {
                use fastrand::usize;
                let db_idx = usize(0..db_names.len());
                let write_start = Instant::now();

                let vectors: Vec<VectorDataDto> = (0..50)
                    .map(|k| VectorDataDto {
                        id: Uuid::new_v4().to_string(),
                        embedding: generate_random_vectors(1, 1024)[0].clone(),
                        metadata: format!("writer_{}_batch_{}_vec_{}", i, j, k),
                    })
                    .collect();

                let insert_req = InsertRequest {
                    nodes: vec![vectors],
                    database: db_names[db_idx].clone(),
                    source: source_name.clone(),
                };

                if let Ok(response) = client
                    .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
                    .json(&insert_req)
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        successful += 1;
                    }
                }

                total_latency += write_start.elapsed();
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            write_success.fetch_add(successful, Ordering::SeqCst);
            (i + 1000, successful, total_latency)
        });

        handles.push(handle);
    }

    let _results = futures::future::join_all(handles).await;
    let total_elapsed = start.elapsed();

    let reads_ok = read_success.load(Ordering::SeqCst);
    let writes_ok = write_success.load(Ordering::SeqCst);

    println!("\n MIXED READ/WRITE WORKLOAD RESULTS:");
    println!("  Total workers: {}", total_workers);
    println!("  Readers: {} ({} queries each)", num_readers, read_queries);
    println!(
        "  Writers: {} ({} inserts each)",
        num_writers, write_queries
    );
    println!("  Total time: {:?}", total_elapsed);
    println!("  Successful reads: {}/{}", reads_ok, num_readers * 20);
    println!("  Successful writes: {}/{}", writes_ok, num_writers * 5);

    assert!(
        reads_ok >= (num_readers * 20 * 95 / 100) as u64,
        "Too many read failures!"
    );
    assert!(
        writes_ok >= (num_writers * 5 * 95 / 100) as u64,
        "Too many write failures!"
    );

    println!("Mixed read/write workload handled successfully!");
}
