use crate::server::{
    CreateDatabaseRequest, CreateSourceRequest, InsertRequest, VectorDataDto, VectorQueryRequest,
};
use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::RngExt;
use reqwest::Client;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Barrier;

const BASE_NUM_DATABASES_WRITE: usize = 100;
const BASE_VECTORS_PER_DB: usize = 768;
const DIMENSIONS: usize = 1024;
const BASE_NUM_CONCURRENT_THUNDERING: usize = 75;
const BASE_NUM_DATABASES_MIXED: usize = 25;
const BASE_NUM_VECTORS_MIXED: usize = 1024;
const BASE_NUM_READERS: usize = 50;
const BASE_READ_QUERIES_PER_READER: usize = 50;
const BASE_NUM_WRITERS: usize = 25;
const BASE_WRITE_QUERIES_PER_WRITER: usize = 10;
const BASE_VECTOR_PER_WRITE_QUERY: usize = 512;

#[inline]
/// Generate a randomized value within ±20% of the base value
fn randomized(base: usize) -> usize {
    let variance = (base as f64 * 0.2) as usize;
    let min = base.saturating_sub(variance);
    let max = base + variance;
    fastrand::usize(min..=max)
}

pub async fn bench_run() -> Result<()> {
    let multi = MultiProgress::new();

    let spinner = multi.add(ProgressBar::new_spinner());
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner} {msg} ({elapsed:.dim})")?,
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message(format!("{}", "Setting up benchmark environment".yellow()));

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let temp_path = temp_dir.path().to_path_buf();

    let config_dir = temp_path.join(".config").join("blaze");
    let sources_dir = temp_path.join("blaze").join("sources");
    tokio::fs::create_dir_all(&config_dir).await?;
    tokio::fs::create_dir_all(&sources_dir).await?;

    let port = find_available_port().await?;
    let base_url = format!("http://127.0.0.1:{}", port);

    // Spawn the server process with isolated HOME
    let mut server_process = spawn_server(&temp_path, port).await?;

    // Wait for server to be ready
    spinner.set_message(format!("{}", "Waiting for server to start...".dimmed()));
    if !wait_for_server(&base_url, 30).await {
        spinner.finish_with_message(format!(
            "{} {}",
            "✗".red().bold(),
            "Server startup failed".red()
        ));
        let _ = server_process.kill().await;
        anyhow::bail!("Server failed to start within timeout");
    }

    spinner.set_message(format!("{}", "Running benchmarks".yellow()));

    // Run benchmarks with the same multi progress
    let results = run_benchmarks(&base_url, &multi).await;

    // Cleanup
    let _ = server_process.kill().await;
    let _ = server_process.wait().await;
    // temp_dir is automatically cleaned up when dropped

    spinner.finish_and_clear();

    match results {
        Ok(stats) => display_results(&stats),
        Err(e) => {
            eprintln!("{} {}", "✗ Benchmark failed:".red().bold(), e);
            return Err(e);
        }
    }

    Ok(())
}

async fn run_benchmarks(base_url: &str, multi: &MultiProgress) -> Result<BenchmarkStats> {
    let mut stats = BenchmarkStats::default();

    // Generate randomized test parameters (±N% variance)
    stats.num_databases_write = randomized(BASE_NUM_DATABASES_WRITE);
    stats.vectors_per_db = randomized(BASE_VECTORS_PER_DB);
    stats.num_concurrent_thunder = randomized(BASE_NUM_CONCURRENT_THUNDERING);
    stats.num_databases_mixed = randomized(BASE_NUM_DATABASES_MIXED);
    stats.num_vectors_mixed = randomized(BASE_NUM_VECTORS_MIXED);
    stats.num_readers = randomized(BASE_NUM_READERS);
    stats.read_queries_per_reader = randomized(BASE_READ_QUERIES_PER_READER);
    stats.num_writers = randomized(BASE_NUM_WRITERS);
    stats.write_queries_per_writer = randomized(BASE_WRITE_QUERIES_PER_WRITER);
    stats.vectors_per_write_query = randomized(BASE_VECTOR_PER_WRITE_QUERY);

    let spinner = multi.add(ProgressBar::new_spinner());
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "∙∙∙∙∙",
                "■∙∙∙∙",
                "◻■∙∙∙",
                "∙◻■∙∙",
                "∙∙◻■∙",
                "∙∙∙◻■",
                "∙∙∙∙◻",
                "∙∙∙∙∙",
                "∙∙∙∙■",
                "∙∙∙■◻",
                "∙∙■◻∙",
                "∙■◻∙∙",
                "■◻∙∙∙",
                "◻∙∙∙∙",
                "∙∙∙∙∙",
            ])
            .template("{spinner:.cyan} {msg}")?,
    );
    spinner.enable_steady_tick(Duration::from_millis(80));

    spinner.set_message(format!("{}", "Concurrent Writes".bold().magenta()));
    run_concurrent_writes_test(base_url, &mut stats).await?;

    spinner.set_message(format!("{}", "Thundering herd".bold().magenta()));
    run_thundering_herd_test(base_url, &mut stats).await?;

    spinner.set_message(format!("{}", "Mixed read/write Workload".bold().magenta()));
    run_mixed_workload_test(base_url, &mut stats).await?;

    spinner.finish_and_clear();
    Ok(stats)
}

async fn spawn_server(temp_dir: &PathBuf, port: u16) -> Result<Child> {
    // Get the path to the blzdb binary
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    let blzdb_path = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory"))?
        .join("blzdb.exe"); // On Windows it's .exe

    // If not found, try without .exe (Unix)
    let blzdb_path = if blzdb_path.exists() {
        blzdb_path
    } else {
        current_exe
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory"))?
            .join("blzdb")
    };

    if !blzdb_path.exists() {
        anyhow::bail!(
            "blzdb binary not found at {}. Make sure to build with: cargo build --bin blzdb",
            blzdb_path.display()
        );
    }

    // Spawn init first
    let init_output = Command::new(&blzdb_path)
        .arg("init")
        .env("BLAZE_HOME", temp_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to spawn blzdb init")?;

    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr);
        anyhow::bail!("blzdb init failed: {}", stderr);
    }

    // Spawn serve
    let mut child = Command::new(&blzdb_path)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .env("BLAZE_HOME", temp_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn blzdb serve")?;

    // Log server output in background
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(_line)) = lines.next_line().await {
                // eprintln!("[server] {}", line.dimmed());
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(_line)) = lines.next_line().await {
                // eprintln!("[server] {}", line.dimmed());
            }
        });
    }

    Ok(child)
}

#[inline]
async fn find_available_port() -> Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

#[inline]
async fn wait_for_server(base_url: &str, max_attempts: u32) -> bool {
    let client = create_client();
    for _ in 0..max_attempts {
        if let Ok(response) = client
            .get(format!("{}/v1/blazedb/health", base_url))
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

fn create_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("Failed to create HTTP client")
}

#[derive(Debug, Default)]
struct BenchmarkStats {
    // Test configuration (actual randomized values)
    num_databases_write: usize,
    vectors_per_db: usize,
    num_concurrent_thunder: usize,
    num_databases_mixed: usize,
    num_vectors_mixed: usize,
    num_readers: usize,
    read_queries_per_reader: usize,
    num_writers: usize,
    write_queries_per_writer: usize,
    vectors_per_write_query: usize,

    // Concurrent writes
    write_total_time: Duration,
    write_min_time: Duration,
    write_max_time: Duration,
    write_avg_time: Duration,
    write_success: u64,
    write_expected: u64,
    write_speedup: f64,

    // Thundering herd
    thunder_total_time: Duration,
    thunder_min_latency: Duration,
    thunder_max_latency: Duration,
    thunder_avg_latency: Duration,
    thunder_success: u64,
    thunder_expected: u64,
    thunder_latency_ratio: f64,

    // Mixed workload
    mixed_total_time: Duration,
    mixed_read_success: u64,
    mixed_read_expected: u64,
    mixed_write_success: u64,
    mixed_write_expected: u64,
}

async fn run_concurrent_writes_test(base_url: &str, stats: &mut BenchmarkStats) -> Result<()> {
    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let num_databases = stats.num_databases_write;
    let vectors_per_db = stats.vectors_per_db;

    // Create source
    let source_name = format!("bench_src_{}", timestamp);

    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };

    client
        .post(format!("{}/v1/blazedb/sources/create", base_url))
        .json(&source_req)
        .send()
        .await
        .context("Failed to create source")?;

    // Create databases
    let mut db_names = vec![];
    for i in 0..num_databases {
        let db_name = format!("bench_db_{}_{}", timestamp, i);

        let db_req = CreateDatabaseRequest {
            name: db_name.clone(),
            source: source_name.clone(),
            metrics: None,
            dimensions: DIMENSIONS,
            backup_interval_hours: None,
        };

        client
            .post(format!("{}/v1/blazedb/databases/create", base_url))
            .json(&db_req)
            .send()
            .await
            .context("Failed to create database")?;

        db_names.push(db_name);
    }

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
        let base_url = base_url.to_string();

        let handle = tokio::spawn(async move {
            barrier.wait().await;
            let write_start = Instant::now();

            // Generate vectors using proper DTO
            let vectors: Vec<VectorDataDto> = (0..vectors_per_db)
                .map(|i| VectorDataDto {
                    embedding: generate_random_vector(DIMENSIONS),
                    metadata: format!("db_{}_vector_{}", idx, i),
                })
                .collect();

            let insert_req = InsertRequest {
                nodes: vec![vectors],
                database: db_name,
                source: source_name,
            };

            let response = client
                .post(format!("{}/v1/blazedb/insert", base_url))
                .json(&insert_req)
                .send()
                .await;

            let elapsed = write_start.elapsed();

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
        .filter_map(|r| r.ok())
        .collect();

    let total_elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);

    stats.write_total_time = total_elapsed;
    stats.write_min_time = *results.iter().min().unwrap_or(&Duration::ZERO);
    stats.write_max_time = *results.iter().max().unwrap_or(&Duration::ZERO);
    stats.write_avg_time = if !results.is_empty() {
        results.iter().sum::<Duration>() / results.len() as u32
    } else {
        Duration::ZERO
    };
    stats.write_success = success;
    stats.write_expected = num_databases as u64;

    // Calculate speedup
    let expected_sequential = stats.write_avg_time * num_databases as u32;
    stats.write_speedup = if total_elapsed.as_secs_f64() > 0.0 {
        expected_sequential.as_secs_f64() / total_elapsed.as_secs_f64()
    } else {
        1.0
    };

    Ok(())
}

async fn run_thundering_herd_test(base_url: &str, stats: &mut BenchmarkStats) -> Result<()> {
    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();
    let num_concurrent = stats.num_concurrent_thunder;

    // Create source and database
    let source_name = format!("bench_src_thunder_{}", timestamp);
    let db_name = format!("bench_db_thunder_{}", timestamp);

    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}/v1/blazedb/sources/create", base_url))
        .json(&source_req)
        .send()
        .await?;

    let db_req = CreateDatabaseRequest {
        name: db_name.clone(),
        source: source_name.clone(),
        metrics: None,
        dimensions: DIMENSIONS,
        backup_interval_hours: None,
    };
    client
        .post(format!("{}/v1/blazedb/databases/create", base_url))
        .json(&db_req)
        .send()
        .await?;

    // Insert initial data (5000 vectors for thundering herd test)
    let vectors: Vec<VectorDataDto> = (0..4096)
        .map(|i| VectorDataDto {
            embedding: generate_random_vector(DIMENSIONS),
            metadata: format!("vector_{}", i),
        })
        .collect();

    let insert_req = InsertRequest {
        nodes: vec![vectors],
        database: db_name.clone(),
        source: source_name.clone(),
    };
    client
        .post(format!("{}/v1/blazedb/insert", base_url))
        .json(&insert_req)
        .send()
        .await?;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let success_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let mut handles = vec![];

    for _ in 0..num_concurrent {
        let client = create_client();
        let db_name = db_name.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let success_count = Arc::clone(&success_count);
        let base_url = base_url.to_string();

        let handle = tokio::spawn(async move {
            barrier.wait().await;
            let query_start = Instant::now();

            let query_req = VectorQueryRequest {
                query_vector: generate_random_vector(DIMENSIONS),
                database: db_name,
                source: source_name,
                top_k: 10,
            };

            let response = client
                .post(format!("{}/v1/blazedb/query/vector", base_url))
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
        .filter_map(|r| r.ok())
        .collect();

    let total_elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);

    stats.thunder_total_time = total_elapsed;
    stats.thunder_min_latency = *results.iter().min().unwrap_or(&Duration::ZERO);
    stats.thunder_max_latency = *results.iter().max().unwrap_or(&Duration::ZERO);
    stats.thunder_avg_latency = if !results.is_empty() {
        results.iter().sum::<Duration>() / results.len() as u32
    } else {
        Duration::ZERO
    };
    stats.thunder_success = success;
    stats.thunder_expected = num_concurrent as u64;

    if stats.thunder_min_latency.as_millis() > 0 {
        stats.thunder_latency_ratio = stats.thunder_max_latency.as_millis() as f64
            / stats.thunder_min_latency.as_millis() as f64;
    }

    Ok(())
}

async fn run_mixed_workload_test(base_url: &str, stats: &mut BenchmarkStats) -> Result<()> {
    let client = create_client();
    let timestamp = chrono::Utc::now().timestamp();

    // Use randomized values
    let num_databases = stats.num_databases_mixed;
    let num_vectors_per_db = stats.num_vectors_mixed;
    let num_readers = stats.num_readers;
    let read_queries = stats.read_queries_per_reader;
    let num_writers = stats.num_writers;
    let write_queries = stats.write_queries_per_writer;
    let vectors_per_write = stats.vectors_per_write_query;

    // Create source and databases
    let source_name = format!("bench_src_mixed_{}", timestamp);
    let source_req = CreateSourceRequest {
        source_name: source_name.clone(),
        backup_interval_hours: None,
    };
    client
        .post(format!("{}/v1/blazedb/sources/create", base_url))
        .json(&source_req)
        .send()
        .await?;

    let mut db_names = vec![];

    for i in 0..num_databases {
        let db_name = format!("bench_db_mixed_{}_{}", timestamp, i);
        let db_req = CreateDatabaseRequest {
            name: db_name.clone(),
            source: source_name.clone(),
            metrics: None,
            dimensions: DIMENSIONS,
            backup_interval_hours: None,
        };
        client
            .post(format!("{}/v1/blazedb/databases/create", base_url))
            .json(&db_req)
            .send()
            .await?;

        // Insert initial data using proper DTOs
        let vectors: Vec<VectorDataDto> = (0..num_vectors_per_db)
            .map(|j| VectorDataDto {
                embedding: generate_random_vector(DIMENSIONS),
                metadata: format!("vector_{}", j),
            })
            .collect();

        let insert_req = InsertRequest {
            nodes: vec![vectors],
            database: db_name.clone(),
            source: source_name.clone(),
        };
        client
            .post(format!("{}/v1/blazedb/insert", base_url))
            .json(&insert_req)
            .send()
            .await?;

        db_names.push(db_name);
    }

    let total_workers = num_readers + num_writers;

    let barrier = Arc::new(Barrier::new(total_workers));
    let read_success = Arc::new(AtomicU64::new(0));
    let write_success = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let mut handles = vec![];

    // Spawn readers
    for _ in 0..num_readers {
        let client = create_client();
        let db_names = db_names.clone();
        let source_name = source_name.clone();
        let barrier = Arc::clone(&barrier);
        let read_success = Arc::clone(&read_success);
        let base_url = base_url.to_string();

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut successful = 0;

            for _ in 0..read_queries {
                use fastrand::usize;
                let db_idx = usize(0..db_names.len());

                let query_req = VectorQueryRequest {
                    query_vector: generate_random_vector(DIMENSIONS),
                    database: db_names[db_idx].clone(),
                    source: source_name.clone(),
                    top_k: 5,
                };

                if let Ok(response) = client
                    .post(format!("{}/v1/blazedb/query/vector", base_url))
                    .json(&query_req)
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        successful += 1;
                    }
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            read_success.fetch_add(successful, Ordering::SeqCst);
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
        let base_url = base_url.to_string();

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut successful = 0;

            for j in 0..write_queries {
                use fastrand::usize;
                let db_idx = usize(0..db_names.len());

                // Use proper DTOs with randomized vectors_per_write
                let vectors: Vec<VectorDataDto> = (0..vectors_per_write)
                    .map(|k| VectorDataDto {
                        embedding: generate_random_vector(DIMENSIONS),
                        metadata: format!("writer_{}_batch_{}_vec_{}", i, j, k),
                    })
                    .collect();

                let insert_req = InsertRequest {
                    nodes: vec![vectors],
                    database: db_names[db_idx].clone(),
                    source: source_name.clone(),
                };

                if let Ok(response) = client
                    .post(format!("{}/v1/blazedb/insert", base_url))
                    .json(&insert_req)
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        successful += 1;
                    }
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            write_success.fetch_add(successful, Ordering::SeqCst);
        });

        handles.push(handle);
    }

    let _ = futures::future::join_all(handles).await;
    let total_elapsed = start.elapsed();

    stats.mixed_total_time = total_elapsed;
    stats.mixed_read_success = read_success.load(Ordering::SeqCst);
    stats.mixed_read_expected = (num_readers * read_queries) as u64;
    stats.mixed_write_success = write_success.load(Ordering::SeqCst);
    stats.mixed_write_expected = (num_writers * write_queries) as u64;

    Ok(())
}

#[inline]
fn display_results(stats: &BenchmarkStats) {
    println!("{}", "BENCHMARK RESULTS".bold().cyan());

    println!("\n{}", "Concurrent Writes".bold());
    println!(
        "   Databases: {} | Vectors/DB: {}",
        stats.num_databases_write, stats.vectors_per_db
    );
    println!(
        "   Total Time: {:.2}s",
        stats.write_total_time.as_secs_f64()
    );
    println!(
        "   Min: {:.3}s | Max: {:.3}s | Avg: {:.3}s",
        stats.write_min_time.as_secs_f64(),
        stats.write_max_time.as_secs_f64(),
        stats.write_avg_time.as_secs_f64()
    );
    let write_status = if stats.write_success == stats.write_expected {
        "✓".green()
    } else {
        "✗".red()
    };
    println!(
        "   Success: {}/{} {}",
        stats.write_success, stats.write_expected, write_status
    );
    println!("   Speedup: {:.1}x (vs sequential)", stats.write_speedup);

    println!("\n{}", "Thundering herd".bold());
    println!("   Concurrent Requests: {}", stats.num_concurrent_thunder);
    println!(
        "   Total Time: {:.2}s",
        stats.thunder_total_time.as_secs_f64()
    );
    println!(
        "   Min: {:.3}s | Max: {:.3}s | Avg: {:.3}s",
        stats.thunder_min_latency.as_secs_f64(),
        stats.thunder_max_latency.as_secs_f64(),
        stats.thunder_avg_latency.as_secs_f64()
    );
    let thunder_status = if stats.thunder_success == stats.thunder_expected {
        "✓".green()
    } else {
        "✗".red()
    };
    println!(
        "   Success: {}/{} {}",
        stats.thunder_success, stats.thunder_expected, thunder_status
    );
    let ratio_ok = stats.thunder_latency_ratio < 5.0;
    println!(
        "   Latency Ratio: {:.1}x {}",
        stats.thunder_latency_ratio,
        if ratio_ok {
            "✓ (good)".green()
        } else {
            "✗ (high)".red()
        }
    );

    println!("\n{}", "Mixed Read/Write Workload".bold());
    println!(
        "   Readers: {} ({} queries each)",
        stats.num_readers, stats.read_queries_per_reader
    );
    println!(
        "   Writers: {} ({} inserts of {} vectors each)",
        stats.num_writers, stats.write_queries_per_writer, stats.vectors_per_write_query
    );
    println!(
        "   Total Time: {:.2}s",
        stats.mixed_total_time.as_secs_f64()
    );
    let read_status = if stats.mixed_read_success >= stats.mixed_read_expected * 95 / 100 {
        "✓".green()
    } else {
        "✗".red()
    };
    let write_status = if stats.mixed_write_success >= stats.mixed_write_expected * 95 / 100 {
        "✓".green()
    } else {
        "✗".red()
    };
    println!(
        "   Successful Reads: {}/{} {}",
        stats.mixed_read_success, stats.mixed_read_expected, read_status
    );
    println!(
        "   Successful Writes: {}/{} {}",
        stats.mixed_write_success, stats.mixed_write_expected, write_status
    );

    let all_passed = stats.write_success == stats.write_expected
        && stats.thunder_success == stats.thunder_expected
        && stats.mixed_read_success >= stats.mixed_read_expected * 95 / 100
        && stats.mixed_write_success >= stats.mixed_write_expected * 95 / 100
        && stats.thunder_latency_ratio < 5.0;

    if all_passed {
    } else {
        println!(
            "{}",
            " Some benchmarks had issues, don't question my code and get a better CPU"
                .bold()
                .yellow()
        );
    }
}

// #[inline]
// fn format_duration(d: Duration) -> String {
//     if d.as_millis() >= 1000 {
//         format!("{:.2}s", d.as_secs_f64())
//     } else {
//         format!("{}ms", d.as_millis())
//     }
// }

#[inline]
fn generate_random_vector(dimensions: usize) -> Vec<f32> {
    let mut rng = rand::rng();
    (0..dimensions)
        .map(|_| rng.random_range(-1.0..1.0))
        .collect()
}
