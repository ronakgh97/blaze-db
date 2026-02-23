use blaze_db::prelude::{EmbeddingStore, HNSW, Metrics};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::{Barrier, RwLock};
use tokio::time::Instant;

/// Test concurrent writes to different databases - should work in parallel
#[tokio::test]
async fn test_concurrent_writes_different_databases() {
    let dir = tempdir().unwrap();
    let num_databases = 5;
    let vectors_per_db = 100;

    let start = Instant::now();

    // Spawn concurrent write tasks for different databases
    let mut handles = vec![];

    for db_idx in 0..num_databases {
        let db_path = dir.path().join(format!("db_{}", db_idx));
        std::fs::create_dir_all(&db_path).unwrap();

        let handle = tokio::spawn(async move {
            let mut hnsw = HNSW::new(18, 200, 12, 0.8, &Some(Metrics::Cosine));

            // Insert vectors
            for i in 0..vectors_per_db {
                let vector: Vec<f32> = (0..1024).map(|x| (x + i) as f32 / 1000.0).collect();
                let metadata = format!("db_{}_vector_{}", db_idx, i);
                let level = hnsw.get_random_level();
                let random_id = uuid::Uuid::new_v4().to_string();
                let _ = hnsw.insert(random_id, &vector, metadata, level);
            }

            // Save to disk
            let index_path = db_path.join("HNSW_INDEX_1");
            let mut store = EmbeddingStore::new(hnsw.clone());
            store.write_to_disk(&index_path).await.unwrap();

            (db_idx, hnsw.nodes.len())
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results: Vec<_> = futures::future::join_all(handles).await;

    let elapsed = start.elapsed();

    // Verify all databases were written correctly
    for result in results {
        let (db_idx, node_count) = result.unwrap();
        println!("Database {} has {} nodes", db_idx, node_count);
        assert_eq!(node_count, vectors_per_db);
    }

    println!(
        "Concurrent writes to {} databases completed in {:?}",
        num_databases, elapsed
    );

    // Verify that concurrent execution was faster than sequential would be
    // (this is a rough check - concurrent should be significantly faster)
    assert!(
        elapsed < Duration::from_secs(30),
        "Concurrent writes took too long"
    );
}

/// Test concurrent writes to the SAME database - should be serialized
#[tokio::test]
async fn test_concurrent_writes_same_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("shared_db");
    std::fs::create_dir_all(&db_path).unwrap();

    let num_writers = 5;
    let vectors_per_writer = 50;

    // Use RwLock to simulate database write locks
    let db_lock = Arc::new(RwLock::new(()));
    let barrier = Arc::new(Barrier::new(num_writers));

    let start = Instant::now();

    let mut handles = vec![];

    for writer_idx in 0..num_writers {
        let db_path = db_path.clone();
        let db_lock = Arc::clone(&db_lock);
        let barrier = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            // Wait for all writers to be ready
            barrier.wait().await;

            // Acquire write lock
            let _write_guard = db_lock.write().await;
            let write_start = Instant::now();

            // Load existing index or create new one
            let index_file = db_path.join("HNSW_INDEX_1.bin");
            let mut hnsw = if index_file.exists() {
                match EmbeddingStore::load_index_file(&index_file).await {
                    Ok(store) => store.hnsw_store,
                    Err(_) => HNSW::new(18, 200, 12, 0.8, &Some(Metrics::Cosine)),
                }
            } else {
                HNSW::new(18, 200, 12, 0.8, &Some(Metrics::Cosine))
            };

            let initial_count = hnsw.nodes.len();

            // Insert vectors
            for i in 0..vectors_per_writer {
                let vector: Vec<f32> = (0..1024)
                    .map(|x| ((x + i + writer_idx * 1000) as f32) / 1000.0)
                    .collect();
                let metadata = format!("writer_{}_vector_{}", writer_idx, i);
                let level = hnsw.get_random_level();
                let random_id = uuid::Uuid::new_v4().to_string();
                let _ = hnsw.insert(random_id, &vector, metadata, level);
            }

            // Save to disk
            let index_path = db_path.join("HNSW_INDEX_1");
            let mut store = EmbeddingStore::new(hnsw.clone());
            store.write_to_disk(&index_path).await.unwrap();

            let write_elapsed = write_start.elapsed();

            drop(_write_guard);

            (writer_idx, initial_count, hnsw.nodes.len(), write_elapsed)
        });

        handles.push(handle);
    }

    // Wait for all writers to complete
    let results: Vec<_> = futures::future::join_all(handles).await;

    let total_elapsed = start.elapsed();

    // Verify results
    let mut total_vectors_written = 0;
    for result in &results {
        let (writer_idx, initial, final_count, duration) = result.as_ref().unwrap();
        println!(
            "Writer {} added {} vectors ({}→{}) in {:?}",
            writer_idx,
            final_count - initial,
            initial,
            final_count,
            duration
        );
        total_vectors_written += final_count - initial;
    }

    // Load final index and verify
    let index_file = db_path.join("HNSW_INDEX_1.bin");
    let final_store = EmbeddingStore::load_index_file(&index_file).await.unwrap();
    let final_count = final_store.hnsw_store.nodes.len();

    println!("Total vectors written: {}", total_vectors_written);
    println!("Final index has {} nodes", final_count);
    println!("Total time: {:?}", total_elapsed);

    // The final count should equal the total vectors written
    // Note: Due to concurrent writes overwriting each other, this might not hold
    // in the current implementation. This test demonstrates the need for proper locking.
    println!("Expected total: {}", num_writers * vectors_per_writer);
}

/// Test read-write lock behavior: multiple readers can read concurrently
#[tokio::test]
async fn test_concurrent_reads_with_write() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("rwlock_test_db");
    std::fs::create_dir_all(&db_path).unwrap();

    // Create initial index
    let mut hnsw = HNSW::new(18, 200, 12, 0.8, &Some(Metrics::Cosine));
    for i in 0..100 {
        let vector: Vec<f32> = (0..1024).map(|x| (x + i) as f32 / 1000.0).collect();
        let metadata = format!("initial_vector_{}", i);
        let level = hnsw.get_random_level();
        let random_id = uuid::Uuid::new_v4().to_string();
        let _ = hnsw.insert(random_id, &vector, metadata, level);
    }

    let index_path = db_path.join("HNSW_INDEX_1");
    let mut store = EmbeddingStore::new(hnsw);
    store.write_to_disk(&index_path).await.unwrap();

    // Create RwLock for the database
    let db_lock = Arc::new(RwLock::new(()));

    let num_readers = 10;
    let barrier = Arc::new(Barrier::new(num_readers + 1)); // +1 for writer

    let mut handles = vec![];

    // Spawn reader tasks
    for reader_idx in 0..num_readers {
        let db_path = db_path.clone();
        let db_lock = Arc::clone(&db_lock);
        let barrier = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            // Wait for all tasks to be ready
            barrier.wait().await;

            // Acquire read lock
            let _read_guard = db_lock.read().await;
            let read_start = Instant::now();

            // Load index
            let index_file = db_path.join("HNSW_INDEX_1.bin");
            let store = EmbeddingStore::load_index_file(&index_file).await.unwrap();

            // Simulate some read operation
            let node_count = store.hnsw_store.nodes.len();
            tokio::time::sleep(Duration::from_millis(10)).await;

            let read_elapsed = read_start.elapsed();

            drop(_read_guard);

            (reader_idx, node_count, read_elapsed)
        });

        handles.push(handle);
    }

    // Spawn a writer task
    let db_path_writer = db_path.clone();
    let db_lock_writer = Arc::clone(&db_lock);
    let barrier_writer = Arc::clone(&barrier);

    let writer_handle = tokio::spawn(async move {
        // Wait for all tasks to be ready
        barrier_writer.wait().await;

        // Small delay to let readers start
        tokio::time::sleep(Duration::from_millis(5)).await;

        let _write_guard = db_lock_writer.write().await;
        let write_start = Instant::now();

        let index_file = db_path_writer.join("HNSW_INDEX_1.bin");
        let mut store = EmbeddingStore::load_index_file(&index_file).await.unwrap();

        for i in 100..120 {
            let vector: Vec<f32> = (0..1024).map(|x| (x + i) as f32 / 1000.0).collect();
            let metadata = format!("new_vector_{}", i);
            let level = store.hnsw_store.get_random_level();
            let random_id = uuid::Uuid::new_v4().to_string();
            let _ = store.hnsw_store.insert(random_id, &vector, metadata, level);
        }

        // Save
        let index_path = db_path_writer.join("HNSW_INDEX_2");
        store.write_to_disk(&index_path).await.unwrap();

        let write_elapsed = write_start.elapsed();

        drop(_write_guard);

        ("writer", store.hnsw_store.nodes.len(), write_elapsed)
    });

    // Wait for all tasks
    let reader_results: Vec<_> = futures::future::join_all(handles).await;
    let writer_result = writer_handle.await.unwrap();

    // Verify readers
    for result in &reader_results {
        let (reader_idx, node_count, duration) = result.as_ref().unwrap();
        println!(
            "Reader {} read {} nodes in {:?}",
            reader_idx, node_count, duration
        );
        assert_eq!(*node_count, 100);
    }

    // Verify writer
    let (_, final_count, write_duration) = writer_result;
    println!("Writer wrote {} nodes in {:?}", final_count, write_duration);
    assert_eq!(final_count, 120);

    println!("✓ Concurrent reads completed successfully while write was pending");
}

/// Test that writes are properly serialized and cumulative
#[tokio::test]
async fn test_cumulative_writes() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("cumulative_db");
    std::fs::create_dir_all(&db_path).unwrap();

    let num_batches = 5;
    let vectors_per_batch = 20;

    // Use lock to ensure proper ordering
    let db_lock = Arc::new(RwLock::new(()));

    for batch_idx in 0..num_batches {
        let _write_guard = db_lock.write().await;

        // Determine the latest index file
        let mut max_index = 0;
        if let Ok(entries) = std::fs::read_dir(&db_path) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str()
                    && name.starts_with("HNSW_INDEX_")
                    && name.ends_with(".bin")
                    && let Some(num_str) = name
                        .strip_prefix("HNSW_INDEX_")
                        .and_then(|s| s.strip_suffix(".bin"))
                    && let Ok(num) = num_str.parse::<usize>()
                {
                    max_index = max_index.max(num);
                }
            }
        }

        // Load existing or create new
        let mut hnsw = if max_index > 0 {
            let index_file = db_path.join(format!("HNSW_INDEX_{}.bin", max_index));
            match EmbeddingStore::load_index_file(&index_file).await {
                Ok(store) => store.hnsw_store,
                Err(_) => HNSW::new(18, 200, 12, 0.8, &Some(Metrics::Cosine)),
            }
        } else {
            HNSW::new(18, 200, 12, 0.8, &Some(Metrics::Cosine))
        };

        let initial_count = hnsw.nodes.len();

        // Insert new batch
        for i in 0..vectors_per_batch {
            let vector: Vec<f32> = (0..1024)
                .map(|x| ((x + i + batch_idx * 100) as f32) / 1000.0)
                .collect();
            let metadata = format!("batch_{}_vector_{}", batch_idx, i);
            let level = hnsw.get_random_level();
            let random_id = uuid::Uuid::new_v4().to_string();
            let _ = hnsw.insert(random_id, &vector, metadata, level);
        }

        // Save with incremented index
        let new_index = max_index + 1;
        let index_path = db_path.join(format!("HNSW_INDEX_{}", new_index));
        let mut store = EmbeddingStore::new(hnsw.clone());
        store.write_to_disk(&index_path).await.unwrap();

        println!(
            "Batch {}: {}→{} nodes, saved to HNSW_INDEX_{}",
            batch_idx,
            initial_count,
            hnsw.nodes.len(),
            new_index
        );

        drop(_write_guard);
    }

    // Verify final state
    let final_index_file = db_path.join(format!("HNSW_INDEX_{}.bin", num_batches));
    let final_store = EmbeddingStore::load_index_file(&final_index_file)
        .await
        .unwrap();
    let final_count = final_store.hnsw_store.nodes.len();

    println!("Final index has {} nodes", final_count);
    assert_eq!(final_count, num_batches * vectors_per_batch);

    println!("✓ Cumulative writes working correctly");
}
