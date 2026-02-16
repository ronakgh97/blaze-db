use crate::core::HNSW;
use anyhow::{Context, Result, anyhow};
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
use tokio::time::Instant;
use wincode::{SchemaRead, SchemaWrite};

#[derive(Serialize, Deserialize, Debug, Clone, SchemaWrite, SchemaRead)]
pub struct EmbeddingStore {
    pub hnsw_store: HNSW,
    // pub checksum: String, What the hell i was thinking here? Stupid me
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct EmbeddingMetadata {
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

    /// Load from a single binary file using memory-mapped I/O for better performance (Thread-safe with blocking task)
    pub async fn load_index_file(path: &PathBuf) -> Result<Self> {
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

            // TODO: Use streaming here for larger indexes
            let config = wincode::config::Configuration::default()
                .with_preallocation_size_limit::<{ 64 * 1024 * 1024 }>();

            // Deserialize from the memory-mapped bytes
            // This creates a zero copy of the data, so it's safe to return across thread boundaries
            let store = wincode::config::deserialize(&mmap[..], config)
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
    pub async fn load_indexes(dir_path: &str) -> Result<Vec<Self>> {
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
                match Self::load_index_file(&path).await {
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

        // TODO: Use streaming here for larger indexes
        let config = wincode::config::Configuration::default()
            .with_preallocation_size_limit::<{ 64 * 1024 * 1024 }>();

        // Serialize to bytes and calculate checksum
        let initial_bytes = wincode::config::serialize(&*self, config)?;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&initial_bytes);
        let checksum = format!("{:x}", hasher.finalize());

        write_metadata(
            &formatted_path
                .parent()
                .ok_or_else(|| anyhow!("File path has no parent directory"))?
                .to_path_buf(), // TODO: Unwrap safe?
            &EmbeddingMetadata {
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

    /// Write the EmbeddingStore to disk as a JSON file and store the hash checksum (for readability and debugging)
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

        write_metadata(
            &formatted_path
                .parent()
                .ok_or_else(|| anyhow!("File path has no parent directory"))?
                .to_path_buf(), // TODO: Unwrap safe?
            &EmbeddingMetadata {
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

    // pub async fn load_lastest_index(prefix: &str, dir_path: &str) -> Result<(Option<Self>, usize)> {
    //     let (loaded_hnsw, max_index) = {
    //         let mut latest_path: Option<PathBuf> = None;
    //         let mut max_num = 0;
    //         for entry in std::fs::read_dir(dir_path)? {
    //             let entry = entry?;
    //             let path = entry.path();
    //             if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
    //                 // Check if it's an index file (e.g., hnsw_index_1.bin)
    //                 if let Some(suffix) = file_name.strip_prefix(prefix) {
    //                     // Remove .bin extension if present
    //                     let suffix = suffix.strip_suffix(".bin").unwrap_or(suffix);
    //                     let suffix = suffix.strip_prefix('_').unwrap_or(suffix);
    //                     // Try to parse the number
    //                     if let Ok(num) = suffix.parse::<usize>() {
    //                         if num > max_num {
    //                             max_num = num;
    //                             latest_path = Some(path);
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //         let loaded = if let Some(path) = latest_path {
    //             Some(EmbeddingStore::load_binary_file(&path).await?)
    //         } else {
    //             None
    //         };
    //         (loaded, max_num)
    //     };
    //
    //     Ok((loaded_hnsw, max_index))
    // }
}

/// Write or update the EmbeddingMetadata on disk, this is auto called when writing the EmbeddingStore
/// Thread-safe with atomic writes using temp-file-rename pattern
pub async fn write_metadata(dir_path: &PathBuf, metadata: &EmbeddingMetadata) -> Result<()> {
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

    #[inline]
    /// Get a value by key
    pub fn get(&self, key: &K) -> Result<Option<V>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.get(key).cloned())
    }

    #[inline]
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

    #[inline]
    /// Check if a key exists
    pub fn contains_key(&self, key: &K) -> Result<bool> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.contains_key(key))
    }

    #[inline]
    /// Get all keys
    pub fn keys(&self) -> Result<Vec<K>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.keys().cloned().collect())
    }

    #[inline]
    /// Get all values
    pub fn values(&self) -> Result<Vec<V>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.values().cloned().collect())
    }

    #[inline]
    /// Get all key-value pairs
    pub fn entries(&self) -> Result<Vec<(K, V)>> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    #[inline]
    /// Get the number of entries
    pub fn len(&self) -> Result<usize> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.len())
    }

    #[inline]
    /// Check if the store is empty
    pub fn is_empty(&self) -> Result<bool> {
        let data = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;

        Ok(data.is_empty())
    }

    #[inline]
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

    #[inline]
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

/// Result of a backup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub timestamp: String,
    pub size_mb: f64,
    pub time_taken_seconds: f64,
}

/// Creates a compressed backup of a single database index file (NOT the entire directory)
/// using a tar archive with zstd compression.
///
/// - Executes compression in a blocking task to avoid stalling the async runtime
/// - Uses atomic file operations (temporary file + rename) to ensure consistency
/// - Thread-safe and safe for concurrent usage
pub async fn create_file_backup(
    backup_dir: &PathBuf,
    file_to_backup: &PathBuf,
    backup_filename: String, // Expected format: my_source_my_database_20240601_153000.tar.zst
    compression_level: i32,
) -> Result<BackupInfo> {
    if !file_to_backup.exists() {
        anyhow::bail!("Specified file does not exist");
    }

    if !backup_dir.is_dir() {
        anyhow::bail!("Backup path is not a valid directory");
    }

    let timestamp = chrono::Utc::now();
    let temp_backup_path = backup_dir.join(format!("{}.tmp", backup_filename));
    let final_backup_path = backup_dir.join(backup_filename);

    // Clone paths for blocking task
    let temp_backup_path_clone = temp_backup_path.clone();
    let file_to_backup_clone = file_to_backup.clone();

    // Perform compression inside a blocking task
    let (size_bytes, elapsed) = tokio::task::spawn_blocking(move || -> Result<(u64, f64)> {
        let start = Instant::now();
        // Create temporary output file
        let temp_file = File::create(&temp_backup_path_clone)
            .with_context(|| format!("Failed to create temp file: {:?}", temp_backup_path_clone))?;

        let encoder = zstd::Encoder::new(temp_file, compression_level)
            .with_context(|| "Failed to initialize zstd encoder")?;

        let mut tar_builder = tar::Builder::new(encoder);

        let file_name = file_to_backup_clone
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("HNSW_INDEX.replica");

        tar_builder
            .append_path_with_name(&file_to_backup_clone, file_name)
            .with_context(|| {
                format!(
                    "Failed to append file to archive: {:?}",
                    file_to_backup_clone
                )
            })?;

        // Finalize tar archive
        let encoder = tar_builder
            .into_inner()
            .with_context(|| "Failed to finalize tar archive")?;

        // Finalize compression
        let mut file = encoder
            .finish()
            .with_context(|| "Failed to finalize zstd compression")?;

        // Ensure data is fully written to disk
        file.flush()
            .with_context(|| "Failed to flush backup file")?;
        file.sync_all()
            .with_context(|| "Failed to sync backup file")?;

        // Retrieve file size
        let metadata = file
            .metadata()
            .with_context(|| "Failed to retrieve backup metadata")?;

        let elapsed = start.elapsed().as_secs_f64();
        Ok((metadata.len(), elapsed))
    })
    .await
    .with_context(|| "Backup task panicked during execution")??;

    // Atomically move temp file to final location
    fs::rename(&temp_backup_path, &final_backup_path)
        .await
        .with_context(|| {
            format!(
                "Failed to rename backup from {:?} to {:?}",
                temp_backup_path, final_backup_path
            )
        })?;

    Ok(BackupInfo {
        file_name: final_backup_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown_backup.tar.zst")
            .to_string(),
        timestamp: timestamp.to_rfc3339(),
        size_mb: size_bytes as f64 / (1024.0 * 1024.0),
        time_taken_seconds: elapsed,
    })
}

/// Creates a compressed backup of multiple specific files using tar + zstd.
///
/// - Backs up only the specified files (not entire directory)
/// - Executes compression in a blocking task
/// - Uses atomic file operations (temp file + rename)
/// - Files are archived with their original filenames
pub async fn create_multi_file_backup(
    backup_dir: &PathBuf,
    files_to_backup: &[PathBuf],
    backup_filename: String,
    compression_level: i32,
) -> Result<BackupInfo> {
    // Validate inputs
    for file in files_to_backup {
        if !file.exists() {
            anyhow::bail!("File to backup does not exist: {:?}", file);
        }
    }

    if !backup_dir.is_dir() {
        anyhow::bail!("Backup directory is not valid");
    }

    let timestamp = chrono::Utc::now();
    let start_time = std::time::Instant::now();

    let temp_backup_path = backup_dir.join(format!("{}.tmp", backup_filename));
    let final_backup_path = backup_dir.join(&backup_filename);

    let temp_backup_path_clone = temp_backup_path.clone();
    let files_clone: Vec<PathBuf> = files_to_backup.to_vec();

    // Perform compression in blocking thread
    let size_bytes = tokio::task::spawn_blocking(move || -> Result<u64> {
        let temp_file = std::fs::File::create(&temp_backup_path_clone)
            .with_context(|| format!("Failed to create temp file: {:?}", temp_backup_path_clone))?;

        let encoder = zstd::Encoder::new(temp_file, compression_level)
            .with_context(|| "Failed to initialize zstd encoder")?;

        let mut tar_builder = tar::Builder::new(encoder);

        // Append each file individually
        for file_path in &files_clone {
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid file name: {:?}", file_path))?;

            tar_builder
                .append_path_with_name(file_path, file_name)
                .with_context(|| format!("Failed to append file to archive: {:?}", file_path))?;
        }

        // Finalize tar
        let encoder = tar_builder
            .into_inner()
            .with_context(|| "Failed to finalize tar archive")?;

        // Finalize compression
        let mut file = encoder
            .finish()
            .with_context(|| "Failed to finalize zstd compression")?;

        file.flush()
            .with_context(|| "Failed to flush backup file")?;
        file.sync_all()
            .with_context(|| "Failed to sync backup file")?;

        let metadata = file
            .metadata()
            .with_context(|| "Failed to retrieve backup metadata")?;

        Ok(metadata.len())
    })
    .await
    .with_context(|| "Backup task panicked during execution")??;

    // Atomic rename
    tokio::fs::rename(&temp_backup_path, &final_backup_path)
        .await
        .with_context(|| {
            format!(
                "Failed to rename backup from {:?} to {:?}",
                temp_backup_path, final_backup_path
            )
        })?;

    let elapsed = start_time.elapsed().as_secs_f64();

    Ok(BackupInfo {
        file_name: final_backup_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown_backup.tar.zst")
            .to_string(),
        timestamp: timestamp.to_rfc3339(),
        size_mb: size_bytes as f64 / (1024.0 * 1024.0),
        time_taken_seconds: elapsed,
    })
}

#[allow(unused)]
/// Creates a compressed backup of an entire directory using tar + zstd.
///
/// - Archives the full directory recursively
/// - Runs compression inside a blocking task
/// - Uses atomic temp file + rename strategy
/// - Ensures data is flushed and synced to disk
pub async fn create_directory_backup(
    backup_dir: &PathBuf,
    directory_to_backup: &PathBuf,
    backup_filename: String, // e.g. my_source_my_database_20240601_153000.tar.zst
    compression_level: i32,
) -> Result<BackupInfo> {
    // Validate inputs
    if !directory_to_backup.is_dir() {
        anyhow::bail!("Source path is not a valid directory");
    }

    if !backup_dir.is_dir() {
        anyhow::bail!("Backup directory is not valid");
    }

    let timestamp = chrono::Utc::now();
    let start_time = std::time::Instant::now();

    let temp_backup_path = backup_dir.join(format!("{}.tmp", backup_filename));
    let final_backup_path = backup_dir.join(&backup_filename);

    let temp_backup_path_clone = temp_backup_path.clone();
    let dir_to_backup_clone = directory_to_backup.to_path_buf();

    // Perform compression in blocking thread
    let size_bytes = tokio::task::spawn_blocking(move || -> Result<u64> {
        let temp_file = std::fs::File::create(&temp_backup_path_clone)
            .with_context(|| format!("Failed to create temp file: {:?}", temp_backup_path_clone))?;

        let encoder = zstd::Encoder::new(temp_file, compression_level)
            .with_context(|| "Failed to initialize zstd encoder")?;

        let mut tar_builder = tar::Builder::new(encoder);

        // Append directory recursively
        tar_builder
            .append_dir_all(".", &dir_to_backup_clone)
            .with_context(|| {
                format!(
                    "Failed to append directory to archive: {:?}",
                    dir_to_backup_clone
                )
            })?;

        // Finalize tar
        let encoder = tar_builder
            .into_inner()
            .with_context(|| "Failed to finalize tar archive")?;

        // Finalize compression
        let mut file = encoder
            .finish()
            .with_context(|| "Failed to finalize zstd compression")?;

        file.flush()
            .with_context(|| "Failed to flush backup file")?;
        file.sync_all()
            .with_context(|| "Failed to sync backup file")?;

        let metadata = file
            .metadata()
            .with_context(|| "Failed to retrieve backup metadata")?;

        Ok(metadata.len())
    })
    .await
    .with_context(|| "Backup task panicked during execution")??;

    // Atomic rename
    tokio::fs::rename(&temp_backup_path, &final_backup_path)
        .await
        .with_context(|| {
            format!(
                "Failed to rename backup from {:?} to {:?}",
                temp_backup_path, final_backup_path
            )
        })?;

    let elapsed = start_time.elapsed().as_secs_f64();

    Ok(BackupInfo {
        file_name: final_backup_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown_backup.tar.zst")
            .to_string(),
        timestamp: timestamp.to_rfc3339(),
        size_mb: size_bytes as f64 / (1024.0 * 1024.0),
        time_taken_seconds: elapsed,
    })
}

/// Remove old backups, by pattern (e.g., "my_source_my_database_{timestamp}") and keep only the latest N backups, where N is configurable (e.g., 5)
///
/// ### Thread Safety
/// - Safe to call concurrently for different databases
/// - Sorts backups by modification time to determine which to delete
pub async fn cleanup_old_backups(
    backup_dir: &PathBuf,
    pattern: &str, // e.g., "my_source_my_database"
    max_backups: usize,
) -> Result<()> {
    let mut backups = Vec::new();

    // Read all backup files for this database
    let mut entries = fs::read_dir(backup_dir)
        .await
        .with_context(|| format!("Failed to read backup directory: {:?}", backup_dir))?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // Match backup files: my_source_my_database_20240601_153000.tar.zst
            if filename.starts_with(pattern) && filename.ends_with(".tar.zst") {
                let metadata = entry.metadata().await?;
                if metadata.is_file() {
                    backups.push((path, metadata.modified()?));
                }
            }
        }
    }

    // If we have more backups than max_backups, delete the oldest ones
    if backups.len() > max_backups {
        // Sort by modification time (newest first)
        backups.sort_by(|a, b| b.1.cmp(&a.1));

        // Delete backups beyond max_backups
        for (old_backup, _) in backups.iter().skip(max_backups) {
            fs::remove_file(old_backup)
                .await
                .with_context(|| format!("Failed to remove old backup: {:?}", old_backup))?;
        }
    }

    Ok(())
}

/// List all backups for a specific database, no filtering, just return all .tar.zst files in the backup directory with their metadata (timestamp, size, etc.)
///
/// ### Thread Safety
/// - Read-only operation, safe to call concurrently
/// - Returns snapshot of available backups at time of call
pub async fn list_database_backups(backup_root: &PathBuf) -> Result<Vec<BackupInfo>> {
    if !backup_root.exists() {
        anyhow::bail!("Backup root directory does not exist: {:?}", backup_root);
    }

    if !backup_root.is_dir() {
        anyhow::bail!("Backup root is not a valid directory: {:?}", backup_root);
    }

    let mut backups = Vec::new();
    let mut entries = fs::read_dir(&backup_root)
        .await
        .with_context(|| format!("Failed to read backup directory: {:?}", backup_root))?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.ends_with(".tar.zst") {
                let metadata = entry.metadata().await?;
                if metadata.is_file() {
                    let size_bytes = metadata.len();
                    let modified = metadata.modified()?;
                    let timestamp: chrono::DateTime<chrono::Utc> = modified.into();

                    backups.push(BackupInfo {
                        file_name: filename.to_string(),
                        timestamp: timestamp.to_rfc3339(),
                        size_mb: size_bytes as f64 / (1024.0 * 1024.0),
                        time_taken_seconds: 0.0,
                    });
                }
            }
        }
    }

    // Sort by timestamp (newest first)
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(backups)
}

/// Restore a database from a backup file (tar + zstd), this will extract the backup to the specified restore path
/// THIS FUNCTION OVERWRITE EXISTING FILES IN THE RESTORE PATH, USE WITH CAUTION!!!
pub async fn restore_database_backup(backup_path: &PathBuf, restore_path: &PathBuf) -> Result<()> {
    // Validate backup file exists
    if !backup_path.exists() {
        anyhow::bail!("Backup file does not exist: {:?}", backup_path);
    }

    if !backup_path.is_file() {
        anyhow::bail!("Backup path is not a file: {:?}", backup_path);
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = restore_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create restore parent directory: {:?}", parent))?;
    }

    // Remove existing directory and files if they exist (overwrite)
    if restore_path.exists() {
        fs::remove_dir_all(restore_path).await.with_context(|| {
            format!(
                "Failed to remove existing database directory: {:?}",
                restore_path
            )
        })?;
    }

    // Create restore directory
    fs::create_dir_all(restore_path)
        .await
        .with_context(|| format!("Failed to create restore directory: {:?}", restore_path))?;

    // Clone paths for blocking task
    let backup_path_clone = backup_path.clone();
    let restore_path_clone = restore_path.clone();

    // Perform tar+zstd decompression in blocking task
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Open backup file
        let backup_file = File::open(&backup_path_clone)
            .with_context(|| format!("Failed to open backup file: {:?}", backup_path_clone))?;

        // Create zstd decoder
        let decoder =
            zstd::Decoder::new(backup_file).with_context(|| "Failed to create zstd decoder")?;

        // Create tar archive reader
        let mut tar_archive = tar::Archive::new(decoder);

        // Extract all files
        tar_archive
            .unpack(&restore_path_clone)
            .with_context(|| format!("Failed to extract backup to: {:?}", restore_path_clone))?;

        Ok(())
    })
    .await
    .with_context(|| "Blocking task panicked during backup restoration")??;

    Ok(())
}

/// Delete a specific backup file
pub async fn delete_backup(backup_path: &PathBuf) -> Result<()> {
    if !backup_path.exists() {
        anyhow::bail!("Backup file does not exist: {:?}", backup_path);
    }

    fs::remove_file(backup_path)
        .await
        .with_context(|| format!("Failed to delete backup file: {:?}", backup_path))?;

    Ok(())
}
