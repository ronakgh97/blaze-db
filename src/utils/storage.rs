use crate::core::HNSW;
use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::fs::File;
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

    /// Load from a single binary file using memory-mapped I/O for better performance.
    ///
    /// ### Thread Safe
    /// This function is fully thread-safe:
    /// - The mmap is created and destroyed within a `spawn_blocking` task
    /// - Deserialization creates an owned copy of the data before returning
    /// - No references to the mmap escape the blocking task
    /// - Multiple concurrent calls are safe as each has its own mmap instance
    ///
    /// ### Performance
    /// Uses `memmap2` for efficient file loading:
    /// - Faster than reading entire file into memory
    /// - Lower memory overhead for large files
    /// - OS-level page cache optimization
    ///
    /// ### Safety
    /// The unsafe `Mmap::map()` call is safe because:
    /// - We only read from the mmap (no writes)
    /// - The file is not modified during the mapping
    /// - The mmap lifetime is scoped to the blocking task
    pub async fn load_binary_file(path: &PathBuf) -> Result<Self> {
        let path_clone = path.to_path_buf();
        let path_for_error = path.to_path_buf();

        let store = tokio::task::spawn_blocking(move || -> Result<Self> {
            // Open file and create memory map
            let file = File::open(&path_clone)
                .with_context(|| format!("Failed to open file: {:?}", path_clone))?;

            // Safety: We're only reading from the mmap, and the file won't be modified while mapped.
            // The mmap is scoped to this blocking task, ensuring no concurrent access issues.
            // The file handle keeps the file open for the duration of the mmap.
            let mmap = unsafe { Mmap::map(&file) }
                .with_context(|| format!("Failed to memory map file: {:?}", path_clone))?;

            // Deserialize from the memory-mapped bytes
            // This creates an owned copy of the data, so it's safe to return across thread boundaries
            let (store, _) = bincode::decode_from_slice(&mmap[..], bincode::config::standard())
                .with_context(|| format!("Failed to deserialize: {:?}", path_clone))?;

            // mmap is automatically unmapped here when it goes out of scope
            Ok(store)
        })
        .await
        .with_context(|| format!("Blocking task panicked while loading: {:?}", path_for_error))??;

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

        // Serialize to bytes and calculate checksum
        let initial_bytes = bincode::encode_to_vec(&*self, bincode::config::standard())?;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&initial_bytes);
        let checksum = format!("{:x}", hasher.finalize());

        // Update checksum in self
        self.checksum = checksum;

        // Serialize again (now with checksum) and write to disk
        let final_bytes = bincode::encode_to_vec(&*self, bincode::config::standard())?;
        fs::write(&formatted_path, &final_bytes)
            .await
            .with_context(|| format!("Failed to write file: {:?}", formatted_path))?;

        Ok(())
    }

    #[allow(unused)]
    pub async fn write_to_disk_json(&mut self, file_path: PathBuf) -> Result<()> {
        // Add extension if not present
        let formatted_path = if file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "json")
            .unwrap_or(false)
        {
            file_path.to_path_buf()
        } else {
            let mut p = file_path.to_path_buf();
            p.set_extension("json");
            p
        };

        let initial_json = serde_json::to_string_pretty(&self)?;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&initial_json);
        let checksum = format!("{:x}", hasher.finalize());

        self.checksum = checksum;

        let final_json = serde_json::to_string_pretty(&self)?;
        fs::write(&formatted_path, &final_json)
            .await
            .with_context(|| format!("Failed to write file: {:?}", formatted_path))?;

        Ok(())
    }

    pub async fn load_lastest_index(prefix: &str, dir_path: &str) -> Result<(Option<Self>, usize)> {
        let (loaded_hnsw, max_index) = {
            let mut latest_path: Option<PathBuf> = None;
            let mut max_num = 0;
            for entry in std::fs::read_dir(dir_path)? {
                let entry = entry?;
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if it's an index file (e.g., hnsw_index_1.bin)
                    if let Some(suffix) = file_name.strip_prefix(prefix) {
                        // Remove .bin extension if present
                        let suffix = suffix.strip_suffix(".bin").unwrap_or(suffix);
                        let suffix = suffix.strip_prefix('_').unwrap_or(suffix);
                        // Try to parse the number
                        if let Ok(num) = suffix.parse::<usize>() {
                            if num > max_num {
                                max_num = num;
                                latest_path = Some(path);
                            }
                        }
                    }
                }
            }
            let loaded = if let Some(path) = latest_path {
                Some(EmbeddingStore::load_binary_file(&path).await?)
            } else {
                None
            };
            (loaded, max_num)
        };

        Ok((loaded_hnsw, max_index))
    }
}

// HELP ME, I CANT UNDERSTAND HOW TO IMPLEMENT LSM TREES AND SSTABLES YET. 😵‍💫

#[allow(dead_code)]
struct LSMTree {}

#[allow(dead_code)]
struct SSTable {}
