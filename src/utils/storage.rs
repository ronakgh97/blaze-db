use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use rayon::iter::ParallelIterator;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

use crate::utils::EmbeddingData;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VectorData {
    pub chunk: Vec<String>,
    pub embedding: Vec<Vec<f32>>,
    pub dimensions: usize,
    pub total_vectors: usize,
}

impl VectorData {
    /// Get a specific vector by index
    pub fn get_vector(&self, index: usize) -> Option<&[f32]> {
        self.embedding.get(index).map(|v| v.as_slice())
    }

    /// Get text chunk by index
    pub fn get_chunk(&self, index: usize) -> Option<&str> {
        self.chunk.get(index).map(|s| s.as_str())
    }

    /// Memory usage estimate in MB
    pub fn memory_usage_mb(&self) -> f64 {
        let vector_bytes: usize = self
            .embedding
            .par_iter()
            .map(|emb| emb.len() * size_of::<f32>())
            .sum();
        let metadata_bytes: usize = self
            .chunk
            .par_iter()
            .map(|c| c.len() * size_of::<u8>())
            .sum();
        (vector_bytes + metadata_bytes) as f64 / (1024.0 * 1024.0)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
pub struct EmbeddingStore {
    pub batch_index: usize,
    pub items: Vec<EmbeddingData>,
}

impl EmbeddingStore {
    pub fn new(batch_index: usize, items: Vec<EmbeddingData>) -> Self {
        Self { batch_index, items }
    }

    pub fn debug_print(&self) {
        println!("Batch Index: {}", self.batch_index);
        self.items.iter().take(3).for_each(|item| {
            println!(
                "Index: {:?}\n Chunk: {:?}\n Embeddings (first 3): {:?}\n Embedding Length : {:?}\n",
                &item.index,
                &item.chunk,
                &item.embedding[..3],
                &item.dimensions,
            );
        });
    }

    /// Load multiple binary files from a directory
    pub async fn read_binary(dir_path: &str) -> Result<VectorData> {
        // Read directory to get all .bin files
        let mut read_dir = fs::read_dir(dir_path)
            .await
            .with_context(|| format!("Failed to read directory: {:?}", dir_path))?;

        let mut bin_files = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "bin")
                .unwrap_or(false)
            {
                bin_files.push(path);
            }
        }

        if bin_files.is_empty() {
            anyhow::bail!("No .bin files found in {:?}", dir_path);
        }

        // Load all files concurrently using tokio tasks
        let mut tasks = Vec::new();
        for path in bin_files {
            let task = tokio::spawn(async move {
                match Self::read_binary_file(&path).await {
                    Ok(store) => Some(store),
                    Err(e) => {
                        eprintln!("Failed to load {:?}: {}", path, e);
                        None
                    }
                }
            });
            tasks.push(task);
        }

        // Await all tasks and collect results
        let mut stores = Vec::new();
        for task in tasks {
            if let Ok(Some(store)) = task.await {
                stores.push(store);
            }
        }

        // Flatten all items from all stores in parallel
        let (all_chunks, all_embeddings): (Vec<String>, Vec<Vec<f32>>) = stores
            .into_par_iter()
            .flat_map(|store| store.items)
            .map(|item| (item.chunk, item.embedding))
            .unzip();

        let dimensions = all_embeddings.first().map(|v| v.len()).unwrap_or(0);
        let total_vectors = all_embeddings.len();

        Ok(VectorData {
            chunk: all_chunks,
            embedding: all_embeddings,
            dimensions,
            total_vectors,
        })
    }

    /// Load from a single binary file
    pub async fn read_binary_file(path: &Path) -> Result<EmbeddingStore> {
        let path_clone = path.to_path_buf();
        let bytes = fs::read(&path_clone)
            .await
            .with_context(|| format!("Failed to read file: {:?}", path_clone))?;

        let path_for_error = path_clone.clone();
        let (store, _) = tokio::task::spawn_blocking(move || {
            bincode::decode_from_slice(&bytes, bincode::config::standard())
        })
        .await?
        .with_context(|| format!("Failed to deserialize: {:?}", path_for_error))?;

        Ok(store)
    }

    /// Write the embedding store to a binary file
    pub async fn write_binary(&self, file_path: &str) -> Result<()> {
        let formatted_path = format!("{}.bin", file_path);
        let self_clone = self.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut file = std::fs::File::create(&formatted_path)?;
            bincode::encode_into_std_write(&self_clone, &mut file, bincode::config::standard())?;
            file.sync_all()?;
            Ok(())
        })
        .await??;
        Ok(())
    }
}

#[allow(dead_code)]
struct LSMTree {}
