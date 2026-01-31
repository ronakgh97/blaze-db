use crate::core::data::{Source, VectorBase};
use crate::utils::DataStore;
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

lazy_static! {
    /// Global thread-safe ServerFile instance
    /// All operations MUST go through this singleton to ensure thread safety
    pub static ref SERVER_FILE: Arc<RwLock<ServerFile>> = {
        let server_file = ServerFile::load_or_new()
            .expect("Failed to initialize ServerFile");
        Arc::new(RwLock::new(server_file))
    };
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientConfig {
    pub url: String,
    pub timeout: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080".to_string(),
            timeout: 30,
        }
    }
}

impl ClientConfig {
    pub fn new(url: String, timeout: u64) -> Self {
        Self { url, timeout }
    }

    pub fn update(&mut self, url: String, timeout: u64) {
        self.url = url;
        self.timeout = timeout;
    }

    /// Load client config from given location
    pub async fn load_config(config_path: &PathBuf) -> Result<ClientConfig> {
        let config_content = fs::read_to_string(&config_path)
            .await
            .with_context(|| format!("Failed to read config file {}", config_path.display()))?;

        let config: ClientConfig = toml::from_str(&config_content)
            .with_context(|| "Failed to parse config".to_string())?;

        Ok(config)
    }

    /// Get default config path
    pub fn get_default_user_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().with_context(|| "No home directory?")?;
        Ok(home.join(".config").join("blaze").join("user_config.toml"))
    }
}

/// SERVER FILE MANAGER (Owns DataStore, provides business logic)
/// ...
/// Manager for all sources and their vector databases
/// This is the ONLY way to interact with server data
///
/// Thread Safety:
/// - Uses DataStore which has internal RwLock for thread-safe operations
/// - Global singleton wrapped in Arc<RwLock<>> for additional safety
/// - All disk I/O is synchronized through DataStore
pub struct ServerFile {
    store: DataStore<String, Source>,
}

impl ServerFile {
    /// Initialize ServerFile by loading from disk or creating new
    /// This is called once during lazy_static initialization
    /// THIS DOES NOT CREATE DEFAULT SOURCES AUTOMATICALLY
    fn load_or_new() -> Result<Self> {
        let path = Self::get_default_server_file_path()?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir: {}", parent.display()))?;
        }

        let store = DataStore::new(path)?;

        // // If no sources exist, create default source
        // if store.is_empty()? {
        //     let default_source = Source::default();
        //     store.insert(default_source.source_name.clone(), default_source.clone())?;
        //
        //     // Create directory on disk for default source
        //     let default_source_path = get_source_path()?.join(&default_source.source_name);
        //     std::fs::create_dir_all(&default_source_path).with_context(|| {
        //         format!(
        //             "Failed to create default source directory: {}",
        //             default_source_path.display()
        //         )
        //     })?;
        // }

        Ok(Self { store })
    }

    /// Get default server file path
    pub fn get_default_server_file_path() -> Result<PathBuf> {
        let home = dirs::home_dir().with_context(|| "No home directory?")?;
        Ok(home.join(".config").join("blaze").join("SERVER_DATA.json"))
    }

    /// Get a source by name
    pub fn get_source(&self, source_name: &str) -> Result<Option<Source>> {
        self.store.get(&source_name.to_string())
    }

    /// List all source names
    pub fn list_sources(&self) -> Result<Vec<String>> {
        self.store.keys()
    }

    /// Get all sources
    pub fn get_all_sources(&self) -> Result<Vec<Source>> {
        self.store.values()
    }

    /// Check if a source exists
    pub fn source_exists(&self, source_name: &str) -> Result<bool> {
        self.store.contains_key(&source_name.to_string())
    }

    /// Add a new source (creates directory on source disk)
    pub async fn add_source(
        &mut self,
        src_id: String,
        source_name: String,
        created_at: String,
    ) -> Result<Source> {
        // Check if already exists
        if self.store.contains_key(&source_name)? {
            anyhow::bail!("Source '{}' already exists", source_name);
        }

        // Create source object
        let source = Source::new(src_id, source_name.clone(), created_at);

        // Add to store (this saves to disk automatically)
        self.store.insert(source_name.clone(), source.clone())?;

        // Create directory on disk
        let source_path = get_source_path()?.join(&source_name);
        fs::create_dir_all(&source_path).await.with_context(|| {
            format!(
                "Failed to create source directory: {}",
                source_path.display()
            )
        })?;

        Ok(source)
    }

    pub async fn add_source_with_generated(&mut self, source_name: String) -> Result<Source> {
        // Check if already exists
        if self.store.contains_key(&source_name)? {
            anyhow::bail!("Source '{}' already exists", source_name);
        }

        let src_id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        // Create source object
        let source = Source::new(src_id, source_name.clone(), created_at);

        // Add to store (this saves to disk automatically)
        self.store.insert(source_name.clone(), source.clone())?;

        // Create directory on disk
        let source_path = get_source_path()?.join(&source_name);
        fs::create_dir_all(&source_path).await.with_context(|| {
            format!(
                "Failed to create source directory: {}",
                source_path.display()
            )
        })?;

        Ok(source)
    }

    /// Update an existing source
    pub fn update_source(&mut self, source: Source) -> Result<()> {
        // Verify source exists before updating
        if !self.store.contains_key(&source.source_name)? {
            anyhow::bail!("Source '{}' does not exist", source.source_name);
        }

        self.store.insert(source.source_name.clone(), source)?;
        Ok(())
    }

    /// Remove a source (optionally delete directory)
    pub async fn remove_source(&mut self, source_name: &str, delete_files: bool) -> Result<Source> {
        // Get source before deleting
        let source = self
            .store
            .get(&source_name.to_string())?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        // Remove from store
        self.store.delete(&source_name.to_string())?;

        // Optionally delete directory
        if delete_files {
            let source_path = get_source_path()?.join(source_name);
            if source_path.exists() {
                fs::remove_dir_all(&source_path).await.with_context(|| {
                    format!(
                        "Failed to delete source directory: {}",
                        source_path.display()
                    )
                })?;
            }
        }

        Ok(source)
    }

    /// Check if source is valid (exists in store AND on disk)
    /// This is the proper validation that checks both sources of truth
    // TODO: PERFORMANCE - This async function does filesystem I/O
    // Often called while holding SERVER_FILE write lock (see database.rs)
    // Future optimization: Cache results with TTL or use separate validation system
    pub async fn is_source_valid(&self, source_name: &str) -> Result<bool> {
        let in_store = self.store.contains_key(&source_name.to_string())?;
        let on_disk = get_source_path()?.join(source_name).exists();
        Ok(in_store && on_disk)
    }

    /// Fast check: only checks if source exists in store (no I/O)
    /// Use this when you already hold a lock and can't do async I/O
    pub fn source_exists_in_store(&self, source_name: &str) -> Result<bool> {
        self.store.contains_key(&source_name.to_string())
    }

    /// Sync sources - reconcile store with filesystem
    /// Useful for detecting manually created/deleted directories
    pub async fn sync_sources(&mut self) -> Result<SyncReport> {
        let mut report = SyncReport::default();

        // Get sources from store
        let stored_sources = self.store.keys()?;

        // Get directories from filesystem
        let source_base_path = get_source_path()?;
        let mut fs_sources = Vec::new();

        if source_base_path.exists() {
            let mut entries = fs::read_dir(&source_base_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    fs_sources.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }

        // Find sources in store but not on filesystem (orphaned entries)
        for source_name in &stored_sources {
            if !fs_sources.contains(source_name) {
                report.orphaned_in_store.push(source_name.clone());
            }
        }

        // Find sources on filesystem but not in store (untracked directories)
        for source_name in &fs_sources {
            if !stored_sources.contains(source_name) {
                report.untracked_on_fs.push(source_name.clone());
            }
        }

        Ok(report)
    }

    /// Create directories for all sources in the store
    pub async fn create_source_dirs(&self) -> Result<()> {
        let sources = self.store.keys()?;

        for source_name in sources {
            let path_buf = get_source_path()?.join(&source_name);
            fs::create_dir_all(&path_buf).await.with_context(|| {
                format!("Failed to create directory for source: {}", source_name)
            })?;
        }

        Ok(())
    }

    /// Add a vector base to a source
    pub fn add_vector_base(&mut self, source_name: &str, vb: VectorBase) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        // Check for duplicate vector base name
        if source.find_vector_base(&vb.vb_name).is_some() {
            anyhow::bail!(
                "VectorBase '{}' already exists in source '{}'",
                vb.vb_name,
                source_name
            );
        }

        source.add_vector_base(vb);
        self.update_source(source)?;

        Ok(())
    }

    /// Get a vector base from a source, returns None if not found
    pub fn get_vector_base(&self, source_name: &str, vb_name: &str) -> Result<Option<VectorBase>> {
        let source = self.get_source(source_name)?;

        match source {
            Some(s) => Ok(s.find_vector_base(vb_name).cloned()),
            None => Ok(None),
        }
    }

    /// Update a vector base in a source
    pub fn update_vector_base(&mut self, source_name: &str, updated_vb: VectorBase) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        if !source.update_vector_base(updated_vb) {
            anyhow::bail!("VectorBase not found in source '{}'", source_name);
        }

        self.update_source(source)?;

        Ok(())
    }

    /// Remove a vector base from a source
    pub fn remove_vector_base(&mut self, source_name: &str, vb_id: &str) -> Result<VectorBase> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        let removed_vb = source
            .remove_vector_base(vb_id)
            .ok_or_else(|| anyhow::anyhow!("VectorBase with id '{}' not found", vb_id))?;

        self.update_source(source)?;

        Ok(removed_vb)
    }

    /// List all vector bases in a source
    pub fn list_vector_bases(&self, source_name: &str) -> Result<Vec<VectorBase>> {
        let source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        Ok(source.vector_bases.clone())
    }

    /// Update vector base node count
    pub fn update_node_count(
        &mut self,
        source_name: &str,
        vb_name: &str,
        count: u32,
    ) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        let vb = source
            .find_vector_base_mut(vb_name)
            .ok_or_else(|| anyhow::anyhow!("VectorBase '{}' not found", vb_name))?;

        vb.set_node_count(count);

        self.update_source(source)?;

        Ok(())
    }

    /// Touch a vector base (update last_accessed_at)
    pub fn touch_vector_base(&mut self, source_name: &str, vb_name: &str) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        let vb = source
            .find_vector_base_mut(vb_name)
            .ok_or_else(|| anyhow::anyhow!("VectorBase '{}' not found", vb_name))?;

        vb.touch();

        self.update_source(source)?;

        Ok(())
    }

    /// Reload data from disk (useful for external modifications)
    pub fn reload(&self) -> Result<()> {
        self.store.reload()
    }

    /// Get a snapshot of all data
    pub fn snapshot(&self) -> Result<HashMap<String, Source>> {
        self.store.snapshot()
    }
}

/// Report from syncing sources with filesystem
#[derive(Debug, Default, Clone)]
pub struct SyncReport {
    /// Sources in store but not on filesystem
    pub orphaned_in_store: Vec<String>,
    /// Directories on filesystem but not in store
    pub untracked_on_fs: Vec<String>,
}

impl SyncReport {
    pub fn is_clean(&self) -> bool {
        self.orphaned_in_store.is_empty() && self.untracked_on_fs.is_empty()
    }
}

/// Get the base path where all sources are stored
pub fn get_source_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home_dir.join("blaze").join("sources"))
}

/// Save TOML configs (kept for backward compatibility with user_config.toml)
pub async fn save_config<T>(config_path: PathBuf, config: &T) -> Result<()>
where
    T: Serialize,
{
    // Create parent directory
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }

    let toml_string = toml::to_string_pretty(&config)
        .with_context(|| format!("Failed to serialize config to {}", config_path.display()))?;

    fs::write(&config_path, toml_string)
        .await
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

    Ok(())
}

/// Check if a source is valid (exists in store AND on disk)
/// This is a convenience function that holds the read lock internally
pub async fn check_source_valid(source_name: &str) -> Result<bool> {
    let server_file = SERVER_FILE.read().await;
    server_file.is_source_valid(source_name).await
}

/// Get a source by name (convenience function), holds read lock internally
#[allow(dead_code)]
pub async fn get_source(source_name: &str) -> Result<Option<Source>> {
    let server_file = SERVER_FILE.read().await;
    server_file.get_source(source_name)
}

/// List all sources (convenience function), holds read lock internally
pub async fn list_sources() -> Result<Vec<String>> {
    let server_file = SERVER_FILE.read().await;
    server_file.list_sources()
}
