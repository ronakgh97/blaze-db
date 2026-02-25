use anyhow::Result;
use blaze_db::core::HNSW;
use blaze_db::prelude::{EmbeddingStore, Metrics, Provider, VectorData};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct StartupDemo {
    name: String,
    images: String,
    alt: String,
    description: String,
    link: String,
    city: String,
}

impl StartupDemo {
    fn to_formatted_string(&self) -> String {
        let desc_or_alt = if self.description.trim().is_empty() {
            &self.alt
        } else {
            &self.description
        };
        format!(
            "{}${}${}${}",
            self.name, desc_or_alt, self.images, self.link
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _data = read_dataset().await?;

    // run_startup_demo_embed().await?;

    let embeddings_path = "embeddings/Startup_EMBEDDINGS.json";

    let index_path = "examples/startup_index/HNSW_INDEX.bin";

    if PathBuf::from(embeddings_path).exists() {
        println!("Embeddings file already exists.");
    } else {
        run_startup_demo_embed().await?;
    }

    if PathBuf::from(index_path).exists() {
        println!("Index file already exists.");
    } else {
        run_startup_index(&PathBuf::from(index_path)).await?;
    }

    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
        "local",
    );

    let query = "Coffee";
    let embedding = provider.fetch_embedding(query).await?;

    let index = EmbeddingStore::load_index_file(&PathBuf::from(index_path)).await?;

    println!("Total nodes in index: {}", index.hnsw_store.nodes.len());

    let results = index
        .hnsw_store
        .search_with_metadata(&embedding.embedding[0], 10, None);

    println!("Search results:");
    for (i, (node_id, score, metadata)) in results.iter().take(5).enumerate() {
        println!(
            "{}. ID: {}, Score: {:.4}\nMetadata: {}",
            i + 1,
            node_id,
            score,
            metadata
        );
    }

    Ok(())
}

async fn read_dataset() -> Result<Vec<StartupDemo>> {
    read_json_from_path(PathBuf::from("datasets/startups_demo_clean.json")).await
}

async fn read_json_from_path(json_path: PathBuf) -> Result<Vec<StartupDemo>> {
    let startups = tokio::task::spawn_blocking(move || -> Result<Vec<StartupDemo>> {
        use memmap2::MmapOptions;
        use std::fs::File;

        let file = File::open(&json_path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let s = std::str::from_utf8(&mmap[..])?;

        let trimmed = s.trim();
        if trimmed.starts_with('[') {
            // JSON array format
            let startups: Vec<StartupDemo> = serde_json::from_str(trimmed)?;
            return Ok(startups);
        }

        // Fall back to JSONL format (Original format)
        let startups = s
            .lines()
            .filter_map(|line| {
                if line.trim().is_empty() {
                    None
                } else {
                    serde_json::from_str(line).ok()
                }
            })
            .collect();

        Ok(startups)
    })
    .await??;

    Ok(startups)
}

#[tokio::test]
async fn clean_and_save_dataset() -> Result<()> {
    let data = read_dataset().await?;
    let original_count = data.len();
    println!("Original dataset size: {}", original_count);

    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unique_data: Vec<StartupDemo> = Vec::new();
    let mut duplicates_removed = 0;
    let mut empty_both_removed = 0;

    for startup in data {
        let name_lower = startup.name.to_lowercase();
        if !seen_names.insert(name_lower) {
            duplicates_removed += 1;
            continue;
        }

        // Skip if both description AND alt are empty
        if startup.description.trim().is_empty() || startup.alt.trim().is_empty() {
            empty_both_removed += 1;
            continue;
        }

        if startup.link.trim().is_empty() {
            empty_both_removed += 1;
            continue;
        }

        unique_data.push(startup);
    }

    println!("Duplicates removed: {}", duplicates_removed);
    println!(
        "Entries with empty description AND alt removed: {}",
        empty_both_removed
    );
    println!("Clean dataset size: {}", unique_data.len());
    println!(
        "Retention rate: {:.1}%",
        (unique_data.len() as f64 / original_count as f64) * 100.0
    );

    // Write clean dataset to new file
    let output_path = PathBuf::from("datasets/startups_demo_clean.json");
    let json_content = serde_json::to_string_pretty(&unique_data)?;
    tokio::fs::write(&output_path, json_content).await?;
    println!("Clean dataset written to: {:?}", output_path);

    Ok(())
}

#[tokio::test]
async fn check_datasets() -> Result<()> {
    let data = read_dataset().await?;

    // Check duplicate names
    use std::collections::HashSet;
    let mut seen_names = HashSet::new();
    let mut duplicate_count = 0;
    for startup in &data {
        if !seen_names.insert(&startup.name) {
            duplicate_count += 1;
        }
    }

    println!("Total unique startup names: {}", seen_names.len());
    println!("Total duplicate startup names: {}", duplicate_count);

    // Check for empty descriptions
    let empty_desc_count = data
        .iter()
        .filter(|s| s.description.trim().is_empty())
        .count();
    println!("Startups with empty descriptions: {}", empty_desc_count);

    // Check for missing links
    let missing_link_count = data.iter().filter(|s| s.link.trim().is_empty()).count();
    println!("Startups with missing links: {}", missing_link_count);

    // Check for missing alt
    let missing_alt_count = data.iter().filter(|s| s.alt.trim().is_empty()).count();
    println!("Startups with missing alt text: {}", missing_alt_count);

    Ok(())
}

async fn run_startup_index(index_path: &Path) -> Result<()> {
    let vector_data =
        VectorData::read_from_disk(&PathBuf::from("embeddings/Startup_EMBEDDINGS.json")).await?;

    let raw_data = read_dataset().await?;

    // Progress bar setup
    let progress_bar = ProgressBar::new(vector_data.chunk.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("Nodes: [{bar:60.cyan/yellow}] {pos}/{len} ({percent}%)")?
            .progress_chars("■>-"),
    );

    let mut hnsw = HNSW::new(16, 200, 16, 0.8, &Some(Metrics::Cosine));
    let mut skipped = 0;

    // Create a reference to avoid cloning in loop
    let data_ref = &raw_data;

    for (embedding, metadata) in vector_data.embedding.iter().zip(vector_data.chunk.iter()) {
        let random_id = uuid::Uuid::new_v4().to_string();
        let random_level = hnsw.get_random_level();

        // Parse the metadata (startup|||description) to get the startup name and find the corresponding startup in raw_data
        let startup_name = metadata.split("|||").next().unwrap_or("").trim();
        let startup_desc = metadata.split("|||").nth(1).unwrap_or("").trim();

        let startup = match get_startup_by_name(startup_name, startup_desc, data_ref).await {
            Ok(s) => s,
            Err(_) => {
                skipped += 1;
                progress_bar.inc(1);
                continue;
            }
        };

        let hnsw_metadata = startup.to_formatted_string();

        hnsw.insert(random_id, embedding, hnsw_metadata, random_level)?;

        progress_bar.inc(1);
    }

    if skipped > 0 {
        println!("Skipped {} entries that couldn't be matched", skipped);
    }

    let mut embedding_store = EmbeddingStore::new(hnsw);

    embedding_store.write_to_disk(index_path).await?;

    Ok(())
}

#[inline]
async fn get_startup_by_name(
    name: &str,
    _startup_desc_ro_alt: &str,
    data: &[StartupDemo],
) -> Result<StartupDemo> {
    // Collect all candidates with matching name (case-insensitive)
    let mut matches: Vec<StartupDemo> = data
        .par_iter()
        .filter(|s| s.name.eq_ignore_ascii_case(name))
        .cloned()
        .collect();

    if matches.len() == 1 {
        return Ok(matches.remove(0));
    }

    matches
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No matches found."))
}

async fn run_startup_demo_embed() -> Result<()> {
    use blaze_db::prelude::{Provider, VectorData};
    use indicatif::{ProgressBar, ProgressStyle};
    use std::path::PathBuf;
    let data = read_dataset().await?;

    let batch_size = 4096;

    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
        "local",
    );

    // Progress bar setup
    let progress_bar = ProgressBar::new(data.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("Nodes: [{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")?
            .progress_chars("■>-"),
    );

    // We have enough RAM bro
    let mut all_chunks: Vec<String> = Vec::with_capacity(data.len());
    let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(data.len());
    let mut dimensions: usize = 0;

    for i in (0..data.len()).step_by(batch_size) {
        let batch = &data[i..(i + batch_size).min(data.len())];

        let texts_to_embed: Vec<String> = batch
            .iter()
            .map(|s| {
                let desc = if s.description.trim().is_empty() {
                    &s.alt
                } else {
                    &s.description
                };
                format!("{}|||{}", s.name, desc)
            })
            .collect();

        progress_bar.inc(batch.len() as u64);

        let batch_data = provider.fetch_embeddings(&texts_to_embed).await?;

        // Accumulate embeddings in memory
        all_chunks.extend(batch_data.chunk);
        all_embeddings.extend(batch_data.embedding);
        if dimensions == 0 {
            dimensions = batch_data.dimensions;
        }
    }

    // Write all data to disk at once
    let output_path = PathBuf::from("embeddings/Startup_EMBEDDINGS.json");
    let final_data = VectorData {
        chunk: all_chunks,
        embedding: all_embeddings,
        dimensions,
    };
    final_data.write_to_disk(&output_path).await?;
    println!("Successfully wrote {} embeddings to disk", final_data.len());

    progress_bar.finish_with_message("Complete");

    Ok(())
}
