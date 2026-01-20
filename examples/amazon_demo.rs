use anyhow::{Context, Result};
use blaze_db::prelude::*;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let mut hnsw_index = HNSW::new(100, 200, 16, 0.8);
    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
    );
    let index_prefix = "amazon_product_index";

    let batch_size = 4096;

    let data_points = load_amazon_reviews_csv("datasets/amazon_products.csv").await?;

    // Batch the data_points
    let batched_data: Vec<Vec<CsvDataPoint>> = data_points
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    println!("\nTotal data points: {}", data_points.len());
    println!("Total batches: {}", batched_data.len());

    // Progress bar setup
    let progress_bar = ProgressBar::new(batched_data.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("Batch: [{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")?
            .progress_chars("●●-"),
    );

    // Create a directory to store embeddings if it doesn't exist
    let dir_path = PathBuf::from("amazon_embeddings");
    if !dir_path.exists() {
        tokio::fs::create_dir_all(&dir_path).await?;
    }

    for (idx, batch) in batched_data.iter().enumerate() {
        let _batch_vector_count = batch.len();

        // Get embeddings for the entire batch
        let vector_embeddings = provider
            .fetch_embeddings(
                &batch
                    .iter()
                    .map(|data_point| data_point.title.clone())
                    .collect::<Vec<String>>(),
            )
            .await?;

        // Insert each embedding from batch into HNSW index
        for (i, vector) in vector_embeddings.embedding.iter().enumerate() {
            let data_point = &batch[i];
            let random_level = hnsw_index.get_random_level();
            hnsw_index.insert(vector.clone(), data_point.title.clone(), random_level);
        }

        // for data_point in batch {
        //     let vector_embedding =
        //         provider.fetch_embedding(&data_point.title).await?.embedding[0].clone();
        //     let random_level = hnsw_index.get_random_level();
        //     hnsw_index.insert(vector_embedding, data_point.title.clone(), random_level);
        // }

        let mut embedding_store = EmbeddingStore::new(hnsw_index.clone());

        let filename = dir_path.join(format!("{}_{}", index_prefix, idx));

        embedding_store
            .write_to_disk(&filename)
            .await
            .with_context(|| format!("Failed to write batch {}", idx))?;

        // println!(
        //     "Batch {} saved ({} vectors, {} total nodes in HNSW)",
        //     idx,
        //     batch_vector_count,
        //     hnsw_index.nodes.len()
        // );

        progress_bar.inc(1);
    }

    progress_bar.finish_with_message("Done");
    // println!();
    // println!("Total data points embedded: {}", data_points.len());
    // println!("Final HNSW index size: {} nodes", hnsw_index.nodes.len());

    Ok(())
}

#[allow(unused)]
async fn explore_csv() -> Result<()> {
    let data_points = load_amazon_reviews_csv("datasets/amazon_products.csv").await?;
    println!("Loaded {} data points from CSV", data_points.len());

    Ok(())
}

#[allow(unused)]
#[derive(Debug, Deserialize, Clone)]
struct CsvDataPoint {
    title: String,
}

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
    let index_prefix = "amazon_product_index";
    let (lastest_index, max_index) =
        EmbeddingStore::load_lastest_index(index_prefix, "amazon_embeddings").await?;

    let hnsw_index = match lastest_index {
        Some(index) => index,
        None => {
            println!("No index found");
            return Ok(());
        }
    };

    println!(
        "Total nodes: {} in HNSW index: {}",
        hnsw_index.hnsw_store.nodes.len(),
        max_index
    );

    assert_eq!(hnsw_index.hnsw_store.nodes.len(), 50 * 4096); // 50 batches of 4096

    Ok(())
}

/// HOLY CRAP THIS IS FAST
#[tokio::test]
async fn bench_search() -> Result<()> {
    let index_prefix = "amazon_product_index";
    let (lastest_index, _max_index) =
        EmbeddingStore::load_lastest_index(index_prefix, "amazon_embeddings").await?;

    let embedding_store = match lastest_index {
        Some(index) => index,
        None => {
            println!("No index found");
            return Ok(());
        }
    };

    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
    );

    let query = "Gaming RTX 4060 Laptop with 165Hz Display";
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
