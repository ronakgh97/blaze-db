use blaze_db::core::Metrics;
use blaze_db::prelude::{EmbeddingStore, HNSW, Ingestor};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_ingest_to_storage_pipeline() {
    // Setup test file
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "This is line 1").unwrap();
    writeln!(file, "This is line 2").unwrap();

    // Test ingestion
    let ingestor = Ingestor::new(&file_path, 8);
    let batches = ingestor.read_line().unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 2);

    // Create HNSW index with mock embeddings
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));

    // Simulate embedding vectors
    let vector1 = vec![1.0, 2.0, 3.0];
    let vector2 = vec![4.0, 5.0, 6.0];

    let random_id1 = Uuid::new_v4().to_string();
    let random_id2 = Uuid::new_v4().to_string();

    let _ = hnsw.insert(random_id1, &*vector1, "null".to_string(), 0);
    let _ = hnsw.insert(random_id2, &*vector2, "null".to_string(), 0);

    let mut store = EmbeddingStore::new(hnsw);

    // Test storage
    let output_path = dir.path().join("embeddings");
    store.write_to_disk(&output_path).await.unwrap();

    // Verify file was created
    let binary_path = format!("{}.bin", output_path.to_str().unwrap());
    assert!(std::path::Path::new(&binary_path).exists());

    // Load and verify
    let loaded_store = EmbeddingStore::load_index_file(&std::path::PathBuf::from(&binary_path))
        .await
        .unwrap();

    assert_eq!(loaded_store.hnsw_store.nodes.len(), 2);
    assert_eq!(loaded_store.hnsw_store.nodes[0].vector, vector1);
    assert_eq!(loaded_store.hnsw_store.nodes[1].vector, vector2);
}

#[tokio::test]
async fn test_multiple_batch_processing() {
    // Setup test file with many lines to force multiple batches
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("large_test.txt");
    let mut file = File::create(&file_path).unwrap();

    // Write 10 lines with batch size 8 to get 2 batches
    for i in 1..=10 {
        writeln!(file, "Line number {}", i).unwrap();
    }

    let ingestor = Ingestor::new(&file_path, 8);
    let batches = ingestor.read_line().unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].len(), 8);
    assert_eq!(batches[1].len(), 2);

    // Create cumulative HNSW index for first batch (8 vectors)
    let mut hnsw1 = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    for i in 0..8 {
        let random_id = Uuid::new_v4().to_string();
        let _ = hnsw1.insert(
            random_id,
            &*vec![i as f32, (i + 1) as f32],
            "null".to_string(),
            0,
        );
    }
    let mut store1 = EmbeddingStore::new(hnsw1.clone());

    // Create cumulative HNSW index for second batch (8 + 2 = 10 vectors)
    let mut hnsw2 = hnsw1.clone();
    for i in 8..10 {
        let random_id = Uuid::new_v4().to_string();
        let _ = hnsw2.insert(
            random_id,
            &*vec![i as f32, (i + 1) as f32],
            "null".to_string(),
            0,
        );
    }
    let mut store2 = EmbeddingStore::new(hnsw2);

    assert_eq!(store1.hnsw_store.nodes.len(), 8);
    assert_eq!(store2.hnsw_store.nodes.len(), 10); // Cumulative

    // Test writing both batches
    let batch1_path = dir.path().join("batch_0");
    let batch2_path = dir.path().join("batch_1");

    store1.write_to_disk(&batch1_path).await.unwrap();
    store2.write_to_disk(&batch2_path).await.unwrap();

    // Verify both files exist
    assert!(std::path::Path::new(&format!("{}.bin", batch1_path.to_str().unwrap())).exists());
    assert!(std::path::Path::new(&format!("{}.bin", batch2_path.to_str().unwrap())).exists());
}

#[tokio::test]
async fn test_empty_file_processing() {
    // Setup empty test file
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("empty.txt");
    File::create(&file_path).unwrap(); // Create empty file

    let ingestor = Ingestor::new(&file_path, 8);
    let batches = ingestor.read_line().unwrap();
    assert_eq!(batches.len(), 0); // No batches for empty file
}

#[tokio::test]
async fn test_unicode_text_processing() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("unicode.txt");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "Hello 世界! This is unicode text.").unwrap();
    writeln!(file, "Café, naïve, résumé - accented characters").unwrap();
    writeln!(file, "😭 Emoji support test 🤧").unwrap();

    let ingestor = Ingestor::new(&file_path, 8);
    let batches = ingestor.read_line().unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 3);

    // Verify unicode text is preserved during ingestion
    assert_eq!(batches[0][0], "Hello 世界! This is unicode text.");
    assert_eq!(batches[0][1], "Café, naïve, résumé - accented characters");
    assert_eq!(batches[0][2], "😭 Emoji support test 🤧");

    // Create HNSW index with test vectors
    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    let random_id1 = Uuid::new_v4().to_string();
    let random_id2 = Uuid::new_v4().to_string();
    let random_id3 = Uuid::new_v4().to_string();
    let _ = hnsw.insert(random_id1, &*vec![1.0, 2.0], "null".to_string(), 0);
    let _ = hnsw.insert(random_id2, &*vec![3.0, 4.0], "null".to_string(), 0);
    let _ = hnsw.insert(random_id3, &*vec![5.0, 6.0], "null".to_string(), 0);

    let mut store = EmbeddingStore::new(hnsw);

    // Test storage and retrieval
    let output_path = dir.path().join("unicode_embeddings");
    store.write_to_disk(&output_path).await.unwrap();

    let binary_path = format!("{}.bin", output_path.to_str().unwrap());
    let loaded_store = EmbeddingStore::load_index_file(&std::path::PathBuf::from(&binary_path))
        .await
        .unwrap();

    // Verify HNSW structure is preserved after serialization/deserialization
    assert_eq!(loaded_store.hnsw_store.nodes.len(), 3);
    assert_eq!(loaded_store.hnsw_store.nodes[0].vector, vec![1.0, 2.0]);
    assert_eq!(loaded_store.hnsw_store.nodes[1].vector, vec![3.0, 4.0]);
    assert_eq!(loaded_store.hnsw_store.nodes[2].vector, vec![5.0, 6.0]);
}

#[tokio::test]
async fn test_large_embedding_dimensions() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "Test with large embedding dimensions").unwrap();

    let ingestor = Ingestor::new(&file_path, 8);
    let batches = ingestor.read_line().unwrap();
    assert_eq!(batches.len(), 1);

    // Create realistic high-dimensional embeddings (like GPT embeddings)
    let embedding_vector = (0..1536).map(|i| i as f32 * 0.01).collect::<Vec<f32>>();

    let mut hnsw = HNSW::new(16, 100, 5, 0.7, &Some(Metrics::Cosine));
    let random_id = Uuid::new_v4().to_string();
    let _ = hnsw.insert(random_id, &*embedding_vector, "null".to_string(), 0);

    assert_eq!(hnsw.nodes.len(), 1);
    assert_eq!(hnsw.nodes[0].vector.len(), 1536);

    // Test storage and retrieval
    let output_path = dir.path().join("large_embeddings");
    let mut store = EmbeddingStore::new(hnsw);
    store.write_to_disk(&output_path).await.unwrap();

    let binary_path = format!("{}.bin", output_path.to_str().unwrap());
    let loaded_store = EmbeddingStore::load_index_file(&std::path::PathBuf::from(&binary_path))
        .await
        .unwrap();

    assert_eq!(loaded_store.hnsw_store.nodes.len(), 1);
    assert_eq!(loaded_store.hnsw_store.nodes[0].vector.len(), 1536);
    assert_eq!(loaded_store.hnsw_store.nodes[0].vector, embedding_vector);
}
