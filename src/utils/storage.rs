use crate::core::HNSW;
use anyhow::{Context, Result, anyhow};
use bincode::{Decode, Encode};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::hash::Hash;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::fs;

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
pub struct EmbeddingStore {
    pub hnsw_store: HNSW,
    // pub checksum: String, What the hell i was thinking here? Stupid me
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct EmbeddingMetadata {
    pub index_version: usize,
    pub checksum: String,
    pub total_vectors: usize,
    pub dimensions: usize,
    pub last_modified: String,
    pub file_size_mb: usize,
}

impl EmbeddingStore {
    pub fn new(hnsw: HNSW) -> Self {
        Self { hnsw_store: hnsw }
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
    /// ### Safety
    /// The unsafe `Mmap::map()` call is safe because:
    /// - We only read from the mmap (no writes)
    /// - The file is not modified during the mapping
    /// - The mmap lifetime is scoped to the blocking task
    pub async fn load_binary_file(path: &PathBuf) -> Result<Self> {
        let path_clone = path.to_path_buf();

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
        .with_context(|| format!("Blocking task panicked while loading: {:?}", path))??;

        Ok(store)
    }

    #[allow(unused)]
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
    pub async fn write_to_disk(&mut self, file_path: &PathBuf, file_index: usize) -> Result<()> {
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

        write_metadata(
            &formatted_path
                .parent()
                .ok_or_else(|| anyhow!("File path has no parent directory"))?
                .to_path_buf(), // TODO: Unwrap safe?
            &EmbeddingMetadata {
                index_version: file_index,
                checksum,
                total_vectors: self.hnsw_store.nodes.len(),
                dimensions: self.hnsw_store.nodes[0].vector.len(), //TODO: Very hacky but works for now (hopes does not panic 🛐)
                last_modified: chrono::Utc::now().to_rfc3339(),
                file_size_mb: initial_bytes.len() / (1024 * 1024),
            },
        )
        .await?;

        // Write serialized bytes to file
        fs::write(&formatted_path, &initial_bytes)
            .await
            .with_context(|| format!("Failed to write file: {:?}", formatted_path))?;

        Ok(())
    }

    #[allow(unused)]
    pub async fn write_to_disk_json(
        &mut self,
        file_path: PathBuf,
        file_index: usize,
    ) -> Result<()> {
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

        write_metadata(
            &formatted_path
                .parent()
                .ok_or_else(|| anyhow!("File path has no parent directory"))?
                .to_path_buf(), // TODO: Unwrap safe?
            &EmbeddingMetadata {
                index_version: file_index,
                checksum,
                total_vectors: self.hnsw_store.nodes.len(),
                dimensions: self.hnsw_store.nodes[0].vector.len(), //TODO: Very hacky but works for now (hopes does not panic 🛐)
                last_modified: chrono::Utc::now().to_rfc3339(),
                file_size_mb: initial_json.len() / (1024 * 1024),
            },
        )
        .await?;

        // Write serialized bytes to file
        fs::write(&formatted_path, &initial_json)
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

/// Write or update the EmbeddingMetadata on disk, this is auto called when writing the EmbeddingStore
/// Thread-safe with atomic writes using temp-file-rename pattern
async fn write_metadata(dir_path: &PathBuf, metadata: &EmbeddingMetadata) -> Result<()> {
    let metadata_path = dir_path.join("metadata.json");
    let metadata = metadata.clone();

    // Use blocking task for synchronous file operations
    tokio::task::spawn_blocking(move || {
        let store: SingleValueStore<EmbeddingMetadata> = SingleValueStore::new(metadata_path)?;
        store.set(metadata)?;
        Ok::<(), anyhow::Error>(())
    })
    .await
    .with_context(|| "Blocking task panicked while writing metadata")??;

    Ok(())
}

#[allow(unused)]
/// Read the EmbeddingMetadata from disk, good for invalidate cache or integrity checks
/// Thread-safe - uses RwLock internally
pub async fn read_embeddings_metadata(path: &PathBuf) -> Result<EmbeddingMetadata> {
    let metadata_path = path.join("metadata.json");

    // Use blocking task for synchronous file operations
    let metadata = tokio::task::spawn_blocking(move || {
        let store: SingleValueStore<EmbeddingMetadata> = SingleValueStore::new(metadata_path)?;
        store.get()
    })
    .await
    .with_context(|| "Blocking task panicked while reading metadata")??;

    Ok(metadata)
}

// This will do simple Thread safe, concurrent CRUD operations on an in-memory HashMap, with BufReader and BufWriter for reading and writing data.
// This is not going to be whole ass Storage engine, just simple Buffer Reader and Writer, I swear 🙃, Please don't get too involved (Maybe later use Btree)
// Lets begin...

/// Thread-safe DataStore with in-memory HashMap and persistent JSON storage
/// Uses Arc<RwLock<T>> for concurrent access and memmap2 for fast reads
#[derive(Clone)]
#[allow(dead_code)] // Will be used for configs (server_file.toml, etc.)
pub struct DataStore<K, V>
where
    K: Eq + Hash + Clone + Serialize + for<'de> Deserialize<'de>,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    /// In-memory storage with thread-safety
    data: Arc<RwLock<HashMap<K, V>>>,
    /// File path for persistence
    path: PathBuf,
}

#[allow(dead_code)] // Will be used for configs in the future
impl<K, V> DataStore<K, V>
where
    K: Eq + Hash + Clone + Serialize + for<'de> Deserialize<'de>,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new DataStore with the given file path
    pub fn new(path: PathBuf) -> Result<Self> {
        let data = Arc::new(RwLock::new(HashMap::new()));
        let store = DataStore { data, path };

        // Load existing data if file exists
        if store.path.exists() {
            store.load_from_disk()?;
        }

        Ok(store)
    }

    /// Insert or update a key-value pair
    // TODO: PERFORMANCE - Every insert triggers full disk write (save_to_disk)
    // This serializes the ENTIRE HashMap to JSON and writes to disk
    // For high-frequency writes, this becomes a bottleneck
    // Use (WAL) or batched commits
    pub fn insert(&self, key: K, value: V) -> Result<Option<V>> {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        let old_value = data.insert(key, value);
        drop(data); // Release lock before disk I/O - IMPORTANT for concurrency

        // TODO: CORRECTNESS vs PERFORMANCE - Disk write happens AFTER lock release
        // This means: in-memory state is updated, but disk might fail
        // Trade-off: Better concurrency (other readers can proceed) vs durability risk
        // If disk write fails, in-memory and disk are out of sync
        self.save_to_disk()?;

        Ok(old_value)
    }

    /// Get a value by key
    pub fn get(&self, key: &K) -> Result<Option<V>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.get(key).cloned())
    }

    /// Delete a key-value pair
    pub fn delete(&self, key: &K) -> Result<Option<V>> {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        let removed = data.remove(key);
        drop(data); // Release lock before disk I/O

        if removed.is_some() {
            self.save_to_disk()?;
        }

        Ok(removed)
    }

    /// Check if a key exists
    pub fn contains_key(&self, key: &K) -> Result<bool> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.contains_key(key))
    }

    /// Get all keys
    pub fn keys(&self) -> Result<Vec<K>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.keys().cloned().collect())
    }

    /// Get all values
    pub fn values(&self) -> Result<Vec<V>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.values().cloned().collect())
    }

    /// Get all key-value pairs
    pub fn entries(&self) -> Result<Vec<(K, V)>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// Get the number of entries
    pub fn len(&self) -> Result<usize> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.len())
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> Result<bool> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.is_empty())
    }

    /// Clear all data
    pub fn clear(&self) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        data.clear();
        drop(data);

        self.save_to_disk()?;

        Ok(())
    }

    /// Save data to disk using BufWriter for efficient writing (Explicitly)
    pub fn save_to_disk(&self) -> Result<()> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create parent directory")?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .context("Failed to open file for writing")?;

        let mut writer = BufWriter::new(file);

        serde_json::to_writer_pretty(&mut writer, &*data)
            .context("Failed to serialize data to JSON")?;

        writer.flush().context("Failed to flush writer")?;

        Ok(())
    }

    /// Load data from disk using memmap2 for fast reading (Explicitly)
    pub fn load_from_disk(&self) -> Result<()> {
        let file = File::open(&self.path).context("Failed to open file for reading")?;

        // Use memmap2 for fast memory-mapped file access
        let mmap = unsafe { Mmap::map(&file).context("Failed to create memory map")? };

        // Deserialize from the memory-mapped data
        let loaded_data: HashMap<K, V> =
            serde_json::from_slice(&mmap).context("Failed to deserialize JSON data")?;

        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        *data = loaded_data;

        Ok(())
    }

    /// Reload data from disk (useful for synchronization)
    pub fn reload(&self) -> Result<()> {
        if self.path.exists() {
            self.load_from_disk()
        } else {
            Ok(())
        }
    }

    /// Get a snapshot of all data (useful for batch operations)
    pub fn snapshot(&self) -> Result<HashMap<K, V>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.clone())
    }

    /// Batch insert multiple key-value pairs
    pub fn batch_insert(&self, entries: Vec<(K, V)>) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        for (key, value) in entries {
            data.insert(key, value);
        }

        drop(data);

        self.save_to_disk()?;

        Ok(())
    }
}

/// Thread-safe SingleValueStore for storing a single value with persistent JSON storage
/// Specialized version of DataStore for single values (like metadata, configs)
/// Uses Arc<RwLock<T>> for concurrent access and atomic writes with temp-file-rename pattern
#[derive(Clone)]
pub struct SingleValueStore<V>
where
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    /// In-memory storage with thread-safety
    data: Arc<RwLock<Option<V>>>,
    /// File path for persistence
    path: PathBuf,
}

impl<V> SingleValueStore<V>
where
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new SingleValueStore with the given file path
    pub fn new(path: PathBuf) -> Result<Self> {
        let data = Arc::new(RwLock::new(None));
        let store = SingleValueStore { data, path };

        // Load existing data if file exists
        if store.path.exists() {
            store.load_from_disk()?;
        }

        Ok(store)
    }

    /// Set the value (replaces existing value)
    pub fn set(&self, value: V) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        *data = Some(value);
        drop(data); // Release lock before disk I/O

        // Persist to disk with atomic write
        self.save_to_disk()?;

        Ok(())
    }

    /// Get the value (returns error if no value exists)
    pub fn get(&self) -> Result<V> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        data.clone()
            .ok_or_else(|| anyhow::anyhow!("No value stored at {:?}", self.path))
    }

    /// Try to get the value (returns None if no value exists)
    #[allow(dead_code)]
    pub fn try_get(&self) -> Result<Option<V>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.clone())
    }

    /// Clear the value
    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        *data = None;
        drop(data);

        // Delete the file
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .with_context(|| format!("Failed to remove file: {:?}", self.path))?;
        }

        Ok(())
    }

    /// Save data to disk using atomic write-then-rename pattern
    /// This ensures that reads never see partial writes
    pub fn save_to_disk(&self) -> Result<()> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create parent directory")?;
        }

        // Write to temporary file first (atomic write pattern)
        let temp_path = self.path.with_extension("tmp");

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)
            .with_context(|| format!("Failed to open temp file: {:?}", temp_path))?;

        let mut writer = BufWriter::new(file);

        serde_json::to_writer_pretty(&mut writer, &*data)
            .context("Failed to serialize data to JSON")?;

        writer.flush().context("Failed to flush writer")?;
        drop(writer); // Ensure file is closed

        // Atomic rename (on most systems, this is atomic even across processes)
        std::fs::rename(&temp_path, &self.path)
            .with_context(|| format!("Failed to rename {:?} to {:?}", temp_path, self.path))?;

        Ok(())
    }

    /// Load data from disk using memmap2 for fast reading
    pub fn load_from_disk(&self) -> Result<()> {
        let file = File::open(&self.path).context("Failed to open file for reading")?;

        // Use memmap2 for fast memory-mapped file access
        let mmap = unsafe { Mmap::map(&file).context("Failed to create memory map")? };

        // Deserialize from the memory-mapped data
        let loaded_data: V =
            serde_json::from_slice(&mmap).context("Failed to deserialize JSON data")?;

        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        *data = Some(loaded_data);

        Ok(())
    }

    /// Reload data from disk (useful for synchronization)
    #[allow(dead_code)]
    pub fn reload(&self) -> Result<()> {
        if self.path.exists() {
            self.load_from_disk()
        } else {
            Ok(())
        }
    }

    /// Update the value using a closure (atomic update)
    #[allow(dead_code)]
    pub fn update<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(Option<V>) -> Option<V>,
    {
        let mut data = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        let current = data.take();
        *data = updater(current);
        drop(data);

        self.save_to_disk()?;

        Ok(())
    }
}

// HELP ME, I CANT UNDERSTAND HOW TO IMPLEMENT LSM TREES AND SSTABLES YET. 😵‍💫

#[allow(dead_code)]
struct LSMTree {}

#[allow(dead_code)]
struct SSTable {}
