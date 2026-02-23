use anyhow::Result;
use blaze_db::prelude::{Provider, VectorData};
use colored::Colorize;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::exit;

pub const BATCH_SIZE: usize = 4096;
pub const EMBED_FILE_NAME: &str = "EMBEDDINGS.json";

#[tokio::main]
async fn main() -> Result<()> {
    let csv_points = load_amazon_product_csv("datasets/amazon_products.csv").await?;

    // Check for duplicate titles
    let mut titles_set = std::collections::HashSet::new();
    let mut duplicate_count = 0;
    for point in &csv_points {
        if !titles_set.insert(&point.title) {
            duplicate_count += 1;
        }
    }
    if duplicate_count > 0 {
        println!("Found {} duplicate titles in CSV data!", duplicate_count);
        exit(0);
    }

    let embeddings_dir = PathBuf::from("embeddings");
    if !embeddings_dir.exists() {
        tokio::fs::create_dir_all(&embeddings_dir).await?;
    }
    println!("Total CSV data points loaded: {}", csv_points.len());

    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
        "local",
    );

    // Resume logic

    println!("Checking for existing embeddings to resume from...");
    // Load embeddings if created earlier
    let embeddings_path = embeddings_dir.join(EMBED_FILE_NAME);
    let mut accumulated_vector_data = match VectorData::read_from_disk(&embeddings_path).await {
        Ok(v) => {
            let file_size_mb = tokio::fs::metadata(&embeddings_path)
                .await
                .map(|m| m.len() as f64 / (1024.0 * 1024.0))
                .unwrap_or(0.0);
            println!(
                "Found embeddings on disk: {:?}, File: {:.2} MB, Data: {:.2} MB, Embeddings: {}, Chunks: {})",
                embeddings_path.file_name().unwrap(),
                file_size_mb,
                v.data_size_mb(),
                v.embedding.len(),
                v.chunk.len()
            );
            v
        }
        Err(e) => {
            println!(
                "No existing embeddings found on disk, starting fresh. Error: {:?}",
                e.to_string().red().dimmed()
            );
            VectorData::new()
        }
    };

    // Cut to the number of already embedded items in csv
    let already_embedded_count = accumulated_vector_data.chunk.len();
    if already_embedded_count > 0 {
        println!(
            "Resuming from previously saved embeddings, already embedded items: {}",
            already_embedded_count
        );
    }

    if already_embedded_count >= csv_points.len() {
        println!("All items already embedded, Nothing to do...");
        return Ok(());
    }

    let csv_points = &csv_points[already_embedded_count..];

    // Process in batches and save to disk
    let total_batches = csv_points.len().div_ceil(BATCH_SIZE);
    println!(
        "Processing {} batches of size {}",
        total_batches, BATCH_SIZE
    );

    for (batch_idx, chunk) in csv_points.chunks(BATCH_SIZE).enumerate() {
        let batch_number = batch_idx + 1;
        let filename = embeddings_dir.join(EMBED_FILE_NAME);

        println!(
            "Processing batch {}/{} ({} items)...",
            batch_number,
            total_batches,
            chunk.len()
        );

        let titles: Vec<String> = chunk.iter().map(|p| p.title.clone()).collect();

        let batch_vector_data = provider.fetch_embeddings(&titles).await?;

        let before_count = accumulated_vector_data.len();
        let batch_len = batch_vector_data.len();

        // Append the new batch to accumulated data
        accumulated_vector_data
            .embedding
            .extend(batch_vector_data.embedding);
        accumulated_vector_data
            .chunk
            .extend(batch_vector_data.chunk);

        let after_count = accumulated_vector_data.len();
        assert_eq!(
            after_count,
            before_count + batch_len,
            "Data count mismatch! Before: {}, After: {}, Batch: {}",
            before_count,
            after_count,
            batch_len
        );

        // Save accumulated data to disk
        accumulated_vector_data.write_to_disk(&filename).await?;

        let file_size_mb = tokio::fs::metadata(&filename)
            .await
            .map(|m| m.len() as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0);
        println!(
            "Batch {}/{} saved to {:?} (File: {:.2} MB, Data: {:.2} MB, Total Count: {})",
            batch_number,
            total_batches,
            filename.file_name().unwrap(),
            file_size_mb,
            accumulated_vector_data.data_size_mb(),
            accumulated_vector_data.len()
        );
    }

    println!(
        "Total embeddings saved: {} (new: {}, previous: {})",
        accumulated_vector_data.len(),
        csv_points.len(),
        already_embedded_count
    );

    Ok(())
}

#[allow(unused)]
#[derive(Debug, Deserialize, Clone)]
struct CsvDataPoint {
    title: String,
}

async fn load_amazon_product_csv(path: &str) -> Result<Vec<CsvDataPoint>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut points = Vec::new();

    for result in rdr.deserialize() {
        let record: CsvDataPoint = result?;
        points.push(record);
    }

    Ok(points)
}
