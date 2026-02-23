use blaze_db::core::Metrics;
use blaze_db::prelude::{EmbeddingStore, HNSW, VectorData};
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::test]
async fn test_embedding_store_creation() {
    // Create a new HNSW index
    let hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

    let store = EmbeddingStore::new(hnsw.clone());

    assert_eq!(store.hnsw_store.nodes.len(), 0);
    assert_eq!(store.hnsw_store.max_neighbors, 16);
    assert_eq!(store.hnsw_store.ef_construction, 100);
    assert_eq!(store.hnsw_store.max_layers, 5);
}

#[tokio::test]
async fn test_write_read_binary() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_embeddings");

    // Create HNSW index with some test vectors
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    let vector1 = vec![1.0, 2.0, 3.0];
    let vector2 = vec![4.0, 5.0, 6.0];

    let random_id1 = uuid::Uuid::new_v4().to_string();
    let random_id2 = uuid::Uuid::new_v4().to_string();

    let _ = hnsw.insert(random_id1, &vector1, "null".to_string(), 0);
    let _ = hnsw.insert(random_id2, &vector2, "null".to_string(), 0);

    let mut store = EmbeddingStore::new(hnsw);

    // Write binary
    store.write_to_disk(&file_path).await.unwrap();

    // Read binary back
    let binary_path = format!("{}.bin", file_path.to_str().unwrap());
    let loaded_store = EmbeddingStore::load_index_file(&PathBuf::from(&binary_path))
        .await
        .unwrap();

    assert_eq!(
        loaded_store.hnsw_store.nodes.len(),
        store.hnsw_store.nodes.len()
    );
    assert_eq!(
        loaded_store.hnsw_store.max_neighbors,
        store.hnsw_store.max_neighbors
    );
    assert_eq!(loaded_store.hnsw_store.nodes[0].vector, vector1);
    assert_eq!(loaded_store.hnsw_store.nodes[1].vector, vector2);
}

#[tokio::test]
async fn test_read_binary_multiple_files() {
    let dir = tempdir().unwrap();
    let embeddings_dir = dir.path().join("embeddings");
    std::fs::create_dir_all(&embeddings_dir).unwrap();

    // Create multiple embedding stores with cumulative HNSW indices
    let mut cumulative_hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

    for i in 0..3 {
        let vector = vec![i as f32, (i + 1) as f32];
        let random_id = uuid::Uuid::new_v4().to_string();
        let _ = cumulative_hnsw.insert(random_id, &vector, "null".to_string(), 0);

        let mut store = EmbeddingStore::new(cumulative_hnsw.clone());
        let file_path = embeddings_dir.join(format!("batch_{}", i));
        store.write_to_disk(&file_path).await.unwrap();
    }

    // Read all files
    let stores = EmbeddingStore::load_indexes(embeddings_dir.to_str().unwrap())
        .await
        .unwrap();

    assert_eq!(stores.len(), 3);
    // First batch has 1 node, second has 2, third has 3 (cumulative)
    assert_eq!(stores[0].hnsw_store.nodes.len(), 1);
    assert_eq!(stores[1].hnsw_store.nodes.len(), 2);
    assert_eq!(stores[2].hnsw_store.nodes.len(), 3);
}

#[tokio::test]
async fn test_read_binary_empty_directory() {
    let dir = tempdir().unwrap();
    let empty_dir = dir.path().join("empty");
    std::fs::create_dir_all(&empty_dir).unwrap();

    let result = EmbeddingStore::load_indexes(empty_dir.to_str().unwrap()).await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No .bin files found")
    );
}

#[tokio::test]
async fn test_read_binary_nonexistent_directory() {
    let result = EmbeddingStore::load_indexes("/nonexistent/directory").await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Failed to read directory")
    );
}

#[test]
fn test_vector_data_get_vector() {
    let vector_data = VectorData {
        chunk: vec!["chunk1".to_string(), "chunk2".to_string()],
        embedding: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        dimensions: 2,
    };

    assert_eq!(vector_data.get_vector(0), Some([1.0, 2.0].as_slice()));
    assert_eq!(vector_data.get_vector(1), Some([3.0, 4.0].as_slice()));
    assert_eq!(vector_data.get_vector(2), None);
}

#[test]
fn test_vector_data_get_chunk() {
    let vector_data = VectorData {
        chunk: vec!["chunk1".to_string(), "chunk2".to_string()],
        embedding: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        dimensions: 2,
    };

    assert_eq!(vector_data.get_chunk(0), Some("chunk1"));
    assert_eq!(vector_data.get_chunk(1), Some("chunk2"));
    assert_eq!(vector_data.get_chunk(2), None);
}

#[test]
fn test_vector_data_memory_usage() {
    let vector_data = VectorData {
        chunk: vec!["test".to_string()],
        embedding: vec![vec![1.0; 100]], // 100 f32 values
        dimensions: 100,
    };

    let memory_mb = vector_data.data_size_mb();
    assert!(memory_mb > 0.0);
    // Should be approximately 400 bytes (100 * 4) + 4 bytes for "test" = ~0.0004 MB
    assert!(memory_mb < 1.0); // Should be less than 1MB
}

#[test]
fn test_vector_data_empty() {
    let vector_data = VectorData {
        chunk: vec![],
        embedding: vec![],
        dimensions: 0,
    };

    assert_eq!(vector_data.get_vector(0), None);
    assert_eq!(vector_data.get_chunk(0), None);
    assert_eq!(vector_data.data_size_mb(), 0.0);
}

#[test]
fn test_hnsw_search_basic() {
    // Test basic HNSW search functionality
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

    let vector1 = vec![1.0, 0.0, 0.0];
    let vector2 = vec![0.0, 1.0, 0.0];
    let vector3 = vec![0.9, 0.1, 0.0]; // Similar to vector1

    let random_id1 = uuid::Uuid::new_v4().to_string();
    let random_id2 = uuid::Uuid::new_v4().to_string();
    let random_id3 = uuid::Uuid::new_v4().to_string();

    let _ = hnsw.insert(random_id1.clone(), &vector1, "null".to_string(), 0);
    let _ = hnsw.insert(random_id2, &vector2, "null".to_string(), 0);
    let _ = hnsw.insert(random_id3, &vector3, "null".to_string(), 0);

    // Search for something similar to vector1
    let query = vec![1.0, 0.0, 0.0];
    let results = hnsw.search(&query, 2, None);

    assert_eq!(results.len(), 2);
    // The most similar should be node 0 (exact match)
    assert_eq!(results[0].0, random_id1);
}

#[test]
fn test_hnsw_node_insertion() {
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

    assert_eq!(hnsw.nodes.len(), 0);
    assert!(hnsw.entry_point.is_none());

    let vector = vec![1.0, 2.0, 3.0];
    let level = 0;
    let random_id = uuid::Uuid::new_v4().to_string();
    let node_id = hnsw
        .insert(random_id, &vector, "null".to_string(), level)
        .unwrap();

    assert_eq!(node_id, 0);
    assert_eq!(hnsw.nodes.len(), 1);
    assert_eq!(hnsw.entry_point, Some(0));
    assert_eq!(hnsw.nodes[0].vector, vector);
}

#[test]
fn test_hnsw_multiple_insertions() {
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

    for i in 0..10 {
        let random_id = uuid::Uuid::new_v4().to_string();
        let vector = vec![i as f32, (i + 1) as f32, (i + 2) as f32];
        let _ = hnsw.insert(random_id, &vector, "null".to_string(), 0);
    }

    assert_eq!(hnsw.nodes.len(), 10);
}

#[test]
fn test_hnsw_empty_search() {
    let hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    let query = vec![1.0, 2.0, 3.0];
    let results = hnsw.search(&query, 5, None);

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_embedding_store_with_checksum() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_checksum");

    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    let random_id = uuid::Uuid::new_v4().to_string();
    let _ = hnsw.insert(random_id, &[1.0, 2.0, 3.0, 4.0, 5.0], "null".to_string(), 0);

    let mut store = EmbeddingStore::new(hnsw);

    // Write to disk (should generate checksum)
    store.write_to_disk(&file_path).await.unwrap();

    // Load it back and verify
    let binary_path = format!("{}.bin", file_path.to_str().unwrap());
    let loaded_store = EmbeddingStore::load_index_file(&PathBuf::from(&binary_path))
        .await
        .unwrap();

    assert_eq!(loaded_store.hnsw_store.nodes.len(), 1);
    assert_eq!(loaded_store.hnsw_store.nodes[0].vector.len(), 5);
}

#[tokio::test]
async fn test_concurrent_file_loading_thread_safety() {
    // This test verifies that memmap2-based loading is thread-safe
    // by loading the same file multiple times concurrently
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_concurrent");

    // Create a test file with some data
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    for i in 0..100 {
        let random_id = uuid::Uuid::new_v4().to_string();
        let vector = vec![i as f32, (i + 1) as f32, (i + 2) as f32];
        let _ = hnsw.insert(random_id, &vector, format!("chunk_{}", i), 0);
    }

    let mut store = EmbeddingStore::new(hnsw);
    store.write_to_disk(&file_path).await.unwrap();

    let binary_path = PathBuf::from(format!("{}.bin", file_path.to_str().unwrap()));

    // Spawn multiple concurrent tasks to load the same file
    let mut tasks = Vec::new();
    for i in 0..10 {
        let path = binary_path.clone();
        let task = tokio::spawn(async move {
            let loaded = EmbeddingStore::load_index_file(&path).await.unwrap();
            (i, loaded)
        });
        tasks.push(task);
    }

    // Await all tasks and verify they all loaded correctly
    let mut results = Vec::new();
    for task in tasks {
        let (idx, store) = task.await.unwrap();
        results.push((idx, store));
    }

    // Verify all 10 tasks loaded the same data correctly
    assert_eq!(results.len(), 10);
    for (_, loaded_store) in results {
        assert_eq!(loaded_store.hnsw_store.nodes.len(), 100);
        assert_eq!(loaded_store.hnsw_store.max_neighbors, 16);
        assert_eq!(loaded_store.hnsw_store.nodes[0].vector.len(), 3);
    }
}

#[tokio::test]
async fn test_concurrent_different_files_loading() {
    // This test verifies concurrent loading of different files
    let dir = tempdir().unwrap();

    // Create multiple test files
    let mut file_paths = Vec::new();
    for i in 0..5 {
        let file_path = dir.path().join(format!("test_file_{}", i));
        let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

        // Each file has a different number of vectors
        for j in 0..((i + 1) * 10) {
            let random_id = uuid::Uuid::new_v4().to_string();
            let vector = vec![j as f32, (j + 1) as f32];
            let _ = hnsw.insert(random_id, &vector, format!("chunk_{}_{}", i, j), 0);
        }

        let mut store = EmbeddingStore::new(hnsw);
        store.write_to_disk(&file_path).await.unwrap();

        file_paths.push(PathBuf::from(format!(
            "{}.bin",
            file_path.to_str().unwrap()
        )));
    }

    // Load all files concurrently
    let mut tasks = Vec::new();
    for (i, path) in file_paths.into_iter().enumerate() {
        let task = tokio::spawn(async move {
            let loaded = EmbeddingStore::load_index_file(&path).await.unwrap();
            (i, loaded)
        });
        tasks.push(task);
    }

    // Verify results
    let mut results = Vec::new();
    for task in tasks {
        let (idx, store) = task.await.unwrap();
        results.push((idx, store));
    }

    // Sort by index to ensure correct order
    results.sort_by_key(|(idx, _)| *idx);

    // Verify each file loaded the correct number of nodes
    for (i, (idx, store)) in results.iter().enumerate() {
        assert_eq!(i, *idx);
        assert_eq!(store.hnsw_store.nodes.len(), (i + 1) * 10);
    }
}
