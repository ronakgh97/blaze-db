use anyhow::Result;
use blaze_db::prelude::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// Adjust this as you need
const NODES_TO_INDEX: usize = 400_000;

pub const BATCH_SIZE: usize = 4096;

#[tokio::main]
async fn main() -> Result<()> {
    let subcommand = std::env::args().nth(1);
    match subcommand.as_deref() {
        Some("embed") => return run_embed().await,
        Some("index") | None => { /* fall through to index */ }
        Some(other) => {
            eprintln!("Unknown subcommand: '{other}'");
            eprintln!("Usage: amazon_demo [embed|index]");
            std::process::exit(1);
        }
    }

    let vector_data =
        VectorData::read_from_disk(&PathBuf::from("./embeddings/Amazon_EMBEDDINGS.bin")).await?;

    let nodes_to_index = vector_data.size().min(NODES_TO_INDEX) as u64;

    println!("Total embeddings: {}", vector_data.size());
    println!("Building HNSW index with {} nodes...", nodes_to_index);
    // Progress bar setup
    let progress_bar = ProgressBar::new(nodes_to_index);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("Nodes: [{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")?
            .progress_chars("●●-"),
    );

    let mut hnsw = HNSW::default();

    let index_path = PathBuf::from("examples/amazon_index/amazon_index");

    let start_indexing = std::time::Instant::now();
    // Just DUMP embeddings into HNSW index 😒
    for i in 0..nodes_to_index as usize {
        let embedding = match vector_data.get_vector(i) {
            Some(v) => v,
            None => break,
        };
        let metadata = match vector_data.get_chunk(i) {
            Some(m) => m,
            None => break,
        };
        let random_id = Uuid::new_v4().to_string();
        let random_level = hnsw.get_random_level();
        hnsw.insert(random_id, embedding, metadata.to_string(), random_level)?;
        progress_bar.inc(1);
    }

    let mut index_store = EmbeddingStore::new(hnsw);

    index_store.write_to_disk(&index_path).await?;

    let duration = start_indexing.elapsed();
    progress_bar.finish_with_message("Index Completed");
    println!("Took: {:?}", duration);
    Ok(())
}

async fn run_embed() -> Result<()> {
    let csv_points = load_amazon_csv("datasets/amazon_products.csv").await?;

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
    let embeddings_path = embeddings_dir.join("Amazon_EMBEDDINGS.bin");
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
        let filename = embeddings_dir.join("Amazon_EMBEDDINGS.bin");

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

async fn load_amazon_csv(file_path: &str) -> Result<Vec<CsvDataPoint>> {
    let mut rdr = csv::Reader::from_path(file_path)?;
    let mut data_points = Vec::with_capacity(14_000_000);

    for result in rdr.deserialize() {
        let record: CsvDataPoint = result?;
        data_points.push(record);
    }

    Ok(data_points)
}

#[tokio::test]
async fn check_csv() -> Result<()> {
    let csv_points = load_amazon_csv("datasets/amazon_products.csv").await?;

    use std::collections::HashSet;
    let mut seen_titles = HashSet::new();
    let mut duplicate_count = 0;

    for point in &csv_points {
        if !seen_titles.insert(&point.title) {
            duplicate_count += 1;
        }
    }

    println!("\nTotal unique titles: {}", seen_titles.len());
    println!("Total duplicate titles: {}", duplicate_count);
    println!("Total CSV rows: {}", csv_points.len());

    Ok(())
}

#[allow(unused)]
#[derive(Debug, Deserialize, Serialize)]
struct CsvDataPoint {
    id: String,
    title: String,
    #[serde(rename = "imgUrl")]
    img_url: String,
    #[serde(rename = "productURL")]
    product_url: String,
    stars: String,
    reviews: String,
    price: String,
    #[serde(rename = "listPrice")]
    list_price: String,
    category_id: String,
    #[serde(rename = "isBestSeller")]
    is_best_seller: String,
    #[serde(rename = "boughtInLastMonth")]
    bought_in_last_month: String,
}

#[tokio::test]
async fn clean_csv() -> Result<()> {
    use std::collections::HashSet;
    let input_path = "datasets/amazon_products.csv";
    let output_path = "datasets/amazon_products.csv";

    let mut rdr = csv::Reader::from_path(input_path)?;
    let mut seen_titles = HashSet::new();
    let mut unique_records = Vec::new();

    let mut total_count = 0;
    let mut duplicate_count = 0;

    for result in rdr.deserialize() {
        total_count += 1;
        let record: CsvDataPoint = result?;

        if seen_titles.insert(record.title.clone()) {
            unique_records.push(record);
        } else {
            duplicate_count += 1;
        }
    }

    println!("Total records: {}", total_count);
    println!("Duplicate records: {}", duplicate_count);
    println!("Unique records: {}", unique_records.len());

    if duplicate_count == 0 {
        println!("No duplicates found");
        return Ok(());
    }

    let mut wtr = csv::Writer::from_path(output_path)?;

    for record in unique_records {
        wtr.serialize(record)?;
    }
    wtr.flush()?;

    println!("Cleaned CSV saved to {}", output_path);
    Ok(())
}

/// HOLY CRAP THIS IS FAST
#[tokio::test]
async fn bench_search() -> Result<()> {
    use colored::Colorize;
    let index = EmbeddingStore::load_index_file(&PathBuf::from("amazon_index/HNSW.bin")).await;

    let embedding_store = match index {
        Ok(index) => index,
        Err(e) => {
            println!("No index found - Error loading index: {}", e);
            return Ok(());
        }
    };

    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
        "local",
    );

    let query = "Gaming RTX 4060 Laptop with 165Hz Display";
    let query_embedding = provider.fetch_embedding(query).await?;

    let top_k = 100;
    let start_time = std::time::Instant::now();
    let search_results =
        embedding_store
            .hnsw_store
            .search_with_metadata(&query_embedding.embedding[0], top_k, None);
    let duration = start_time.elapsed();

    let start_brute = std::time::Instant::now();
    let brute_results = embedding_store
        .hnsw_store
        .brute_force_search_with_metadata(&query_embedding.embedding[0], top_k);
    let brute_duration = start_brute.elapsed();

    println!("\nQuery: {}", query.to_string().blue());
    println!("Search completed in: {:?}", duration);
    println!("Top {} search results for query: '{}'", top_k, query);
    for (i, (node_id, score, metadata)) in search_results.iter().take(10).enumerate() {
        println!(
            "{}. ID: {}, Score: {:.4}\nTitle: {}",
            i + 1,
            node_id.to_string().cyan(),
            score.to_string().red(),
            metadata.to_string().dimmed().green()
        );
    }

    println!();
    println!("Brute search results (for comparison)");
    println!("Brute search completed in: {:?}", brute_duration);
    for (i, (node_id, score, metadata)) in brute_results.iter().take(10).enumerate() {
        println!(
            "{}. ID: {}, Score: {:.4}\nTitle: {}",
            i + 1,
            node_id.to_string().cyan(),
            score.to_string().red(),
            metadata.to_string().dimmed().green()
        );
    }

    println!(
        "\nSpeedup: {:.2}x",
        brute_duration.as_secs_f64() / duration.as_secs_f64()
    );

    assert!(duration.as_millis() <= 5); // Ensure search is under 5ms

    assert!(duration.as_millis() < brute_duration.as_millis(), "WTF???"); // Ensure HNSW search is faster than brute-force

    Ok(())
}
