use blaze_db::prelude::Ingestor;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_line_vs_smart_chunking_comparison() {
    // Create a test file with War and Peace-like content
    let test_content = r#"The Philosophy of War

War is not merely a political act, but a real political instrument.
A continuation of political intercourse.
A carrying out of the same by other means.

The conduct of War is a matter of strategy and tactics.
Strategy forms the plan of the War.
Tactics deal with the deployment of forces in battle.

Peace without War is impossible in the nature of human society.
Wars should be celebrated as moments of historical transformation.
Because it is the win against the forces of tyranny and oppression.

The great generals of history understood this fundamental truth.
Napoleon, Alexander, Caesar - all recognized war as an instrument.
Not of destruction, but of creation and renewal.

Throughout centuries, conflicts have shaped civilizations.
From the ancient battles of Greece to modern warfare.
Each war has left its mark on human progress."#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(test_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let ingestor = Ingestor::new(temp_file.path(), 8);

    // Old method: line-by-line
    let line_batches = ingestor.read_line().unwrap();
    let total_lines: usize = line_batches.iter().map(|b| b.len()).sum();

    // New method: smart chunking
    let smart_batches = ingestor.read_chunks(100, 50).unwrap();
    let total_chunks: usize = smart_batches.iter().map(|b| b.len()).sum();

    println!("\nCHUNKING COMPARISON");
    println!("Original text: {} chars", test_content.len());
    println!("\nLine-by-Line Method");
    println!("Total chunks: {}", total_lines);
    println!(
        "Average chunk size: ~{} chars",
        test_content.len() / total_lines
    );
    println!("\nFirst 2 chunks:");
    for (i, chunk) in line_batches[0].iter().take(2).enumerate() {
        println!("  {}. \"{}\"", i + 1, chunk);
    }

    println!("\nOverlap Chunking Method");
    println!("Total chunks: {}", total_chunks);
    println!(
        "Average chunk size: ~{} chars",
        test_content.len() / total_chunks
    );
    println!("\nFirst 2 chunk:");
    for (i, chunk) in smart_batches[0].iter().take(2).enumerate() {
        println!("  {}. \"{}\"", i + 1, chunk);
    }

    // Assertions
    assert!(
        total_chunks < total_lines,
        "Overlap chunking should create fewer chunks"
    );
    assert!(total_chunks > 0, "Should create at least one chunk");

    // Each smart chunk should be significantly larger
    if let Some(batch) = smart_batches.first() {
        if let Some(first_chunk) = batch.first() {
            let avg_line_length = test_content.len() / total_lines;
            assert!(
                first_chunk.len() > avg_line_length * 3,
                "Overlap chunks should be much larger than individual lines"
            );
        }
    }

    println!(
        "Overlap chunking creates {} fewer chunks while preserving context!",
        total_lines - total_chunks
    );
}

#[test]
fn test_chunk_overlap() {
    let test_content = r#"First paragraph here.
This is part of the first paragraph.

Second paragraph starts now.
This continues the second paragraph.

Third paragraph is different.
It contains unique information."#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(test_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let ingestor = Ingestor::new(temp_file.path(), 8);

    // Use small chunk size to force multiple chunks
    let chunks = ingestor.read_chunks(20, 5).unwrap();
    let all_chunks: Vec<String> = chunks.into_iter().flatten().collect();

    println!("\nOVERLAP TEST");
    for (i, chunk) in all_chunks.iter().enumerate() {
        println!("Chunk {}: {}", i + 1, chunk);
    }

    // If we have multiple chunks, they should share some words
    if all_chunks.len() > 1 {
        let words_chunk_0: Vec<&str> = all_chunks[0].split_whitespace().collect();
        let words_chunk_1: Vec<&str> = all_chunks[1].split_whitespace().collect();

        // Find common words (overlap)
        let common_count = words_chunk_0
            .iter()
            .filter(|w| words_chunk_1.contains(w))
            .count();

        println!(
            "\nOverlap detected: {} common words between adjacent chunks",
            common_count
        );
        assert!(common_count > 0, "Adjacent chunks should have word overlap");
    }
}
