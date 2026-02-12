use anyhow::Result;
use blaze_db::prelude::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;

#[allow(unused)]
const BATCH_SIZE: usize = 4096;

#[allow(unused)]
const NODES_TO_INDEX: usize = 50_000;

// TODO: Implement Resume logic and batch-wise indexing
#[tokio::main]
async fn main() -> Result<()> {
    let vector_data =
        VectorData::read_from_disk(&PathBuf::from("embeddings/EMBEDDINGS.json")).await?;

    assert_eq!(
        vector_data.embedding.len(),
        vector_data.chunk.len(),
        "Embeddings and Chunks length mismatch!"
    );

    let nodes_to_index = vector_data.embedding.len().min(NODES_TO_INDEX) as u64;

    println!("Total embeddings: {}", vector_data.embedding.len());
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
    for (embedding, metadata) in vector_data
        .embedding
        .iter()
        .take(nodes_to_index as usize)
        .zip(vector_data.chunk.iter())
    {
        let random_level = hnsw.get_random_level();
        hnsw.insert(embedding, metadata.to_string(), random_level);
        progress_bar.inc(1);
    }

    let mut index_store = EmbeddingStore::new(hnsw);

    index_store.write_to_disk(&index_path).await?;

    let duration = start_indexing.elapsed();
    progress_bar.finish_with_message("Index Completed");
    println!("Took: {:?}", duration);
    Ok(())
}

#[allow(unused)]
#[derive(Debug, Deserialize, Clone)]
struct CsvDataPoint {
    title: String,
}

#[allow(unused)]
async fn load_amazon_reviews_csv(file_path: &str) -> Result<Vec<CsvDataPoint>> {
    let mut rdr = csv::Reader::from_path(file_path)?;
    let mut data_points = Vec::with_capacity(10_000_000);

    for result in rdr.deserialize() {
        let record: CsvDataPoint = result?;
        data_points.push(record);
    }

    Ok(data_points)
}

#[tokio::test]
async fn index_health_check() -> Result<()> {
    let hnsw_index =
        EmbeddingStore::load_index_file(&PathBuf::from("examples/amazon_index/amazon_index.bin"))
            .await?;

    println!(
        "Total nodes: {} in HNSW index",
        hnsw_index.hnsw_store.nodes.len(),
    );

    let csv_points = load_amazon_reviews_csv("datasets/amazon_products.csv").await?;

    // Take first N * BATCH_SIZE rows from csv
    let check_points = csv_points[..NODES_TO_INDEX].to_vec();

    // Get all metadata from index
    let metadata_vec: Vec<String> = hnsw_index
        .hnsw_store
        .nodes
        .clone()
        .iter()
        .map(|v| v.metadata.clone())
        .collect();

    use rayon::prelude::*;
    check_points.par_iter().enumerate().for_each(|(i, point)| {
        let matches = metadata_vec
            .iter()
            .filter(|meta| *meta == &point.title)
            .count();
        if matches == 0 {
            panic!("Missing metadata in index: {}", point.title);
        } else if matches > 1 {
            println!(
                "Warning: Duplicate metadata for point {}: {}",
                i, point.title
            );
        }
    });

    Ok(())
}

#[tokio::test]
async fn check_csv() -> Result<()> {
    let csv_points = load_amazon_reviews_csv("datasets/amazon_products.csv").await?;

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

/// HOLY CRAP THIS IS FAST
#[tokio::test]
async fn bench_search() -> Result<()> {
    use colored::Colorize;
    let index =
        EmbeddingStore::load_index_file(&PathBuf::from("examples/amazon_index/amazon_index.bin"))
            .await;

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

    let query = "Valentine gift for girlfriend"; // My lonely ahh ass
    let query_embedding = provider.fetch_embedding(query).await?;

    let top_k = 100;
    let start_time = std::time::Instant::now();
    let search_results = embedding_store
        .hnsw_store
        .search_with_metadata(&query_embedding.embedding[0], top_k);
    let duration = start_time.elapsed();

    println!("\nQuery: {}", query.to_string().blue());
    println!("Search completed in: {:?}", duration);
    println!("Top {} search results for query: '{}'", top_k, query);
    for (i, (node_id, score, metadata)) in search_results.iter().enumerate() {
        println!(
            "{}. ID: {}, Score: {:.4}\nTitle: {}",
            i + 1,
            node_id.to_string().cyan(),
            score.to_string().red(),
            metadata.to_string().dimmed().green()
        );
    }

    assert!(duration.as_millis() <= 5); // Ensure search is under 5ms

    Ok(())
}
