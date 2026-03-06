use anyhow::Result;
use blaze_db::prelude::{Provider, VectorData};
use serde::Deserialize;
use std::path::PathBuf;

pub const BATCH_SIZE: usize = 4096;
pub const EMBED_FILE_NAME: &str = "Amazon_EMBEDDINGS.bin";

#[tokio::main]
async fn main() -> Result<()> {
    let csv_points = load_amazon_product_csv("datasets/amazon_products.csv").await?;

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
            println!(
                "Found embeddings on disk: {:?}, Data: {:.2} MB, Count: {})",
                embeddings_path.file_name().unwrap(),
                v.data_size(),
                v.size()
            );
            v
        }
        Err(e) => {
            println!(
                "No existing embeddings found on disk, starting fresh. Error: {:?}",
                e
            );
            VectorData::new()
        }
    };

    let already_embedded_count = accumulated_vector_data.size();
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

    // Cut to the number of already embedded items in csv
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

        let before_count = accumulated_vector_data.size();
        let batch_len = batch_vector_data.size();

        accumulated_vector_data
            .embedding
            .extend(batch_vector_data.embedding);
        accumulated_vector_data
            .chunk
            .extend(batch_vector_data.chunk);
        accumulated_vector_data.dimensions = batch_vector_data.dimensions;

        let after_count = accumulated_vector_data.size();
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

        println!(
            "Batch {}/{} saved to {:?} Data: {:.2} MB, Total Count: {})",
            batch_number,
            total_batches,
            filename.file_name().unwrap(),
            accumulated_vector_data.data_size(),
            accumulated_vector_data.size()
        );
    }

    println!(
        "Total embeddings saved: {} (new: {}, previous: {})",
        accumulated_vector_data.size(),
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
