use crate::core::HNSW;
use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::PathBuf;
use tokio::fs;

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
pub struct EmbeddingStore {
    pub hnsw_store: HNSW,
    pub checksum: String,
}

impl EmbeddingStore {
    pub fn new(hnsw: HNSW) -> Self {
        Self {
            hnsw_store: hnsw,
            checksum: String::new(),
        }
    }

    /// Get information about the EmbeddingStore
    pub fn get_info(&self) {
        // TODO: What to do here? idk...
        unimplemented!("get_info method not implemented yet");
    }

    /// Load from a single binary file
    pub async fn load_binary_file(path: &PathBuf) -> Result<Self> {
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

    /// Load multiple binary files from a directory //TODO: Will this ever be needed?
    pub async fn load_binaries(dir_path: &str) -> Result<Vec<Self>> {
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
                match Self::load_binary_file(&path).await {
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

        Ok(stores)
    }

    /// Write the EmbeddingStore to disk and store the hash checksum
    pub async fn write_to_disk(&mut self, file_path: &PathBuf) -> Result<()> {
        // Add extension if not present
        let formatted_path = if file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "bin")
            .unwrap_or(false)
        {
            file_path.to_path_buf()
        } else {
            let mut p = file_path.to_path_buf();
            p.set_extension("bin");
            p
        };

        // First, serialize to bytes and calculate checksum
        let self_clone = self.clone(); // TODO: This is so Bad, find a better way
        let checksum = tokio::task::spawn_blocking(move || -> Result<String> {
            let bytes = bincode::encode_to_vec(&self_clone, bincode::config::standard())?;
            let mut hasher = sha2::Sha256::new();
            hasher.update(&bytes);
            let checksum = format!("{:x}", hasher.finalize());
            Ok(checksum)
        })
        .await??;

        // Update checksum in self
        self.checksum = checksum;

        // Now write the updated struct (with checksum) to disk
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

// HELP ME, I CANT UNDERSTAND HOW TO IMPLEMENT LSM TREES AND SSTABLES YET. 😵‍💫

#[allow(dead_code)]
struct LSMTree {}

#[allow(dead_code)]
struct SSTable {}
