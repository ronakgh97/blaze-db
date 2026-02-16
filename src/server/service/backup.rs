use crate::core::SERVER_FILE;
use crate::server::controller::DB_WRITE_LOCKS;
use crate::server::service::database::search_database_on_disk;
use crate::utils::{
    BackupInfo, cleanup_old_backups, create_multi_file_backup, delete_backup as delete_backup_file,
    list_database_backups, read_embeddings_metadata, restore_database_backup,
};
use crate::{error, info, warn};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

/// Global backup state to track ongoing backups and prevent conflicts
pub struct BackupState {
    /// Track ongoing backups per database (source_name:database_name -> lock)
    ongoing_backups: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

impl BackupState {
    pub fn new() -> Self {
        Self {
            ongoing_backups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Try to acquire backup lock for a database
    /// Returns the lock guard if successful, error if backup already in progress
    /// The guard must be held for the entire duration of the backup operation
    pub async fn try_acquire_backup_lock(
        &self,
        db_key: &str,
    ) -> Result<tokio::sync::OwnedMutexGuard<()>> {
        let lock = {
            let mut backups = self.ongoing_backups.write().await;
            backups
                .entry(db_key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // Try to acquire lock without waiting (fail fast)
        match lock.try_lock_owned() {
            Ok(guard) => Ok(guard),
            Err(_) => anyhow::bail!("Backup already in progress for {}", db_key),
        }
    }

    /// Check if a backup is currently running for this database
    pub async fn is_backup_running(&self, db_key: &str) -> bool {
        let backups = self.ongoing_backups.read().await;
        if let Some(lock) = backups.get(db_key) {
            lock.try_lock().is_err()
        } else {
            false
        }
    }

    /// Clean up locks for databases that are no longer in use
    /// Removes entries from the HashMap where only the HashMap itself holds a reference
    pub async fn cleanup_unused_locks(&self) {
        let mut backups = self.ongoing_backups.write().await;

        // Remove locks that have no other Arc references (strong_count == 1 means only HashMap has it)
        backups.retain(|_, lock| Arc::strong_count(lock) > 1);
    }
}

impl Default for BackupState {
    fn default() -> Self {
        Self::new()
    }
}

/// Backup service that handles both scheduled and manual backups
pub struct BackupService {
    state: BackupState,
    config: BackupConfig,
    scheduler_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Configuration for backup operations
#[derive(Clone, Debug)]
pub struct BackupConfig {
    /// Default backup interval in hours
    pub default_interval_hours: u32,
    /// Maximum number of backups to keep per database
    pub max_backups_per_database: usize,
    /// zstd compression level (1-21)
    pub compression_level: i32,
    /// Base directory for backups
    pub backup_base_dir: PathBuf,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            default_interval_hours: 48,
            max_backups_per_database: 5,
            compression_level: 18,
            backup_base_dir: if let Ok(blaze_home) = std::env::var("BLAZE_HOME") {
                PathBuf::from(blaze_home).join("blaze").join("backups")
            } else {
                dirs::home_dir()
                    .map(|h| h.join("blaze").join("backups"))
                    .unwrap_or_else(|| PathBuf::from("/tmp/blaze_backups"))
            },
        }
    }
}

impl BackupService {
    /// Create a new backup service with the given configuration
    pub fn new(config: BackupConfig) -> Self {
        Self {
            state: BackupState::new(),
            config,
            scheduler_handle: None,
        }
    }

    /// Start the background backup scheduler
    pub async fn start_scheduler(&mut self) {
        info!(
            "Starting backup scheduler with {} hour interval",
            self.config.default_interval_hours
        );

        let config = self.config.clone();
        let state = self.state.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Check every 5 minutes

            loop {
                interval.tick().await;

                if let Err(e) = Self::run_scheduled_backups(&config, &state).await {
                    error!("Scheduled backup run failed: {}", e);
                }
            }
        });

        self.scheduler_handle = Some(handle);
    }

    #[inline]
    /// Stop the background scheduler
    pub async fn stop_scheduler(&mut self) {
        if let Some(handle) = self.scheduler_handle.take() {
            info!("Stopping backup scheduler...");
            handle.abort();
        }
    }

    /// Run scheduled backups for all databases that are due
    async fn run_scheduled_backups(config: &BackupConfig, state: &BackupState) -> Result<()> {
        // Periodic cleanup of unused locks (every 100 runs ≈ 8 hours at 5-minute intervals)
        static CLEANUP_COUNTER: AtomicU64 = AtomicU64::new(0);
        if CLEANUP_COUNTER.fetch_add(1, Ordering::Relaxed) % 100 == 0 {
            state.cleanup_unused_locks().await;
        }

        let server_file = SERVER_FILE.read().await;
        let sources = server_file.get_all_sources()?;
        drop(server_file); // Release lock before I/O

        for source in sources {
            let source_interval = if source.backup_interval_hours == 0 {
                config.default_interval_hours as i32
            } else {
                source.backup_interval_hours
            };

            for vb in &source.vector_bases {
                let interval = if vb.backup_interval_hours == 0 {
                    source_interval
                } else {
                    vb.backup_interval_hours
                };

                if interval == -1 {
                    continue; // Backups disabled for this database
                }

                let db_key = format!("{}:{}", source.source_name, vb.vb_name);

                // Check if backup is due
                let should_backup = if let Some(will_backup_at) = &vb.will_backup_at {
                    match chrono::DateTime::parse_from_rfc3339(will_backup_at) {
                        Ok(backup_time) => {
                            chrono::Utc::now() >= backup_time.with_timezone(&chrono::Utc)
                        }
                        Err(_) => {
                            warn!("Invalid backup schedule timestamp for {}", db_key);
                            // TODO: Backup anyway to fix the timestamp
                            // This is ok now, this is backup on first scheduler run, on 5 min window, after that it will follow the schedule properly
                            // Proper way is set will_backup_at to creation time + interval + schedular window time, then schedular wont back it just for sake for fixing the timestamp
                            true
                        }
                    }
                } else {
                    // No schedule set, backup now and set schedule
                    true
                };

                if should_backup && !state.is_backup_running(&db_key).await {
                    info!("Scheduled backup due for {}", db_key);

                    let source_name = source.source_name.clone();
                    let database_name = vb.vb_name.clone();
                    let config = config.clone();
                    let state = state.clone();

                    // Spawn backup task with proper locking
                    tokio::spawn(async move {
                        // Acquire backup lock in the spawned task
                        let db_key = format!("{}:{}", source_name, database_name);

                        // Try to acquire lock, return early if unavailable (panic-safe)
                        let _backup_guard = match state.try_acquire_backup_lock(&db_key).await {
                            Ok(guard) => {
                                info!("Scheduled backup starting for {} (lock acquired)", db_key);
                                guard
                            }
                            Err(e) => {
                                // Backup already in progress (race condition with manual backup)
                                info!("Scheduled backup for {} skipped: {}", db_key, e);
                                return;
                            }
                        };

                        // Execute backup - lock is held even if this panics
                        if let Err(e) =
                            Self::execute_backup(&config, &state, &source_name, &database_name)
                                .await
                        {
                            error!("Scheduled backup failed for {}: {}", db_key, e);
                        }
                        // Lock released automatically when _backup_guard is dropped
                    });
                }
            }
        }

        Ok(())
    }

    #[inline]
    /// Trigger a manual backup via API
    /// Holds backup lock for entire duration to prevent concurrent backups of same database
    pub async fn trigger_backup(&self, source: &str, database: &str) -> Result<BackupInfo> {
        let db_key = format!("{}:{}", source, database);

        // Check if write operation is in progress
        if Self::is_write_in_progress(source, database).await {
            anyhow::bail!("Cannot backup: write operation in progress for {}", db_key);
        }

        // Try to acquire backup lock - this guard MUST be held for entire backup operation
        // The lock is released automatically when guard is dropped
        let _backup_guard = self
            .state
            .try_acquire_backup_lock(&db_key)
            .await
            .context("Backup already in progress")?;

        info!("Starting manual backup for {} (lock acquired)", db_key);

        // Execute backup - lock is held for entire duration
        // This prevents multiple backups of the same database running simultaneously
        let result = Self::execute_backup(&self.config, &self.state, source, database).await;

        // Lock is automatically released here when _backup_guard is dropped
        result
    }

    #[inline]
    /// Check if a write operation is currently in progress for this database
    async fn is_write_in_progress(source: &str, database: &str) -> bool {
        let db_key = format!("{}:{}", source, database);
        let locks = DB_WRITE_LOCKS.read().await;

        if let Some(lock) = locks.peek(&db_key) {
            let is_locked = lock.try_write().is_err();
            info!(
                "Backup check for {}: Lock exists, Try_write={})",
                db_key,
                if is_locked { "LOCKED" } else { "FREE" }
            );
            // is_locked // Can't acquire write lock = someone is writing
        } else {
            info!("Backup check for {}: no lock entry in LRU", db_key);
            // false // No lock exists = no write in progress
        }
        false
        // Since we are using .replica for backup, we can allow backup to proceed even if write is in progress
        // Writes update both .bin and .replica
        // so backup will get a consistent snapshot from .replica even if writes are happening concurrently.
        // This is a advantage of the CoW approach with .replica files.
        // TODO: For strong safe, we could use exponential retry logic here: if write in progress, wait a bit and check again, up to a timeout.
        //  This allows backups to proceed shortly after writes complete without manual intervention, while still preventing backups during active writes.
    }

    /// Execute the actual backup operation
    async fn execute_backup(
        config: &BackupConfig,
        _state: &BackupState,
        source: &str,
        database: &str,
    ) -> Result<BackupInfo> {
        let start_time = std::time::Instant::now();

        // Find database path
        let database_path = search_database_on_disk(database, source)
            .await
            .context("Database not found on disk")?;

        // TODO: For stronger consistency, consider backing up from HNSW_INDEX.bin
        // while holding DB_WRITE_LOCKS.read() instead of using .replica. This prevents any
        // chance of backing up stale data during CoW operations. Trade-off: blocks writes
        // during backup vs current CoW approach which allows writes.

        // Check for replica file - create on-demand from .bin if missing
        // This handles the case where CoW copy failed (disk full, etc.) but .bin exists
        let replica_file = database_path.join("HNSW_INDEX.replica");
        let bin_file = database_path.join("HNSW_INDEX.bin");

        if !replica_file.exists() {
            // TODO: What happens if any deletion happened here?, we need more proper locks, preferably at the file level
            if bin_file.exists() {
                // CoW copy failed previously, create .replica on-demand from .bin
                info!(
                    ".replica missing for {}:{}, creating from .bin for backup",
                    source, database
                );
                tokio::fs::copy(&bin_file, &replica_file)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to create .replica from .bin for backup of {}:{}",
                            source, database
                        )
                    })?;
                info!("Successfully created .replica on-demand for backup");
            } else {
                anyhow::bail!(
                    "No index files found for backup. Ensure database has been written at least once."
                );
            }
        }

        // Check for metadata file - if it doesn't exist, we can't backup properly
        let metadata_file = database_path.join("metadata.json");
        if !metadata_file.exists() {
            anyhow::bail!(
                "No metadata.json found for backup. Ensure database has been written at least once."
            );
        }

        // Create backup directory structure: ~/blaze/backups/{source}/{database}/
        let backup_dir = config.backup_base_dir.join(source).join(database);
        tokio::fs::create_dir_all(&backup_dir)
            .await
            .context("Failed to create backup directory")?;

        // Generate backup filename: backup_YYYYMMDD_HHMMSS.tar.zst
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_filename = format!("backup_{}.tar.zst", timestamp);

        let files_to_backup = vec![replica_file, metadata_file];
        let backup_info = create_multi_file_backup(
            &backup_dir,
            &files_to_backup,
            backup_filename.clone(),
            config.compression_level,
        )
        .await
        .context("Failed to create backup file")?;

        // Cleanup old backups (keep only last N)
        let pattern = "backup_"; // Pattern for backup files
        if let Err(e) =
            cleanup_old_backups(&backup_dir, pattern, config.max_backups_per_database).await
        {
            warn!("Failed to cleanup old backups: {}", e);
        }

        // Update last_backup_at and will_backup_at in SERVER_FILE
        let elapsed = start_time.elapsed().as_secs_f64();
        let next_backup = chrono::Utc::now()
            + chrono::Duration::hours(
                Self::get_effective_interval_hours(config, source, database).await as i64,
            );

        {
            let mut server_file = SERVER_FILE.write().await;
            if let Some(mut src) = server_file.get_source(source)? {
                if let Some(vb) = src.find_vector_base_mut(database) {
                    vb.last_backup_at = Some(chrono::Utc::now().to_rfc3339());
                    vb.will_backup_at = Some(next_backup.to_rfc3339());
                    server_file.update_source(src)?;
                }
            }
        }

        info!(
            "Backup completed for {}:{} - {} ({} MB, {:.2}s)",
            source, database, backup_filename, backup_info.size_mb, elapsed
        );

        Ok(backup_info)
    }

    #[inline]
    /// Get effective backup interval for a database
    async fn get_effective_interval_hours(
        config: &BackupConfig,
        source: &str,
        database: &str,
    ) -> i32 {
        let server_file = SERVER_FILE.read().await;

        if let Ok(Some(src)) = server_file.get_source(source) {
            let source_interval = if src.backup_interval_hours == 0 {
                config.default_interval_hours as i32
            } else {
                src.backup_interval_hours
            };

            if let Some(vb) = src.find_vector_base(database) {
                // If 0, use source interval; if -1, return -1 (disabled)
                if vb.backup_interval_hours == 0 {
                    return source_interval;
                }
                return vb.backup_interval_hours;
            }

            return source_interval;
        }

        config.default_interval_hours as i32
    }

    /// List all backups for a database
    pub async fn list_backups(&self, source: &str, database: &str) -> Result<Vec<BackupInfo>> {
        let backup_dir = self.config.backup_base_dir.join(source).join(database);

        if !backup_dir.exists() {
            return Ok(vec![]);
        }

        list_database_backups(&backup_dir).await
    }

    /// Restore database from a backup (DESTRUCTIVE - replaces current index)
    ///
    /// NOTE: We DON'T need DB_WRITE_LOCKS here because:
    /// - Backups work with .replica files (isolated from queries)
    /// - Restore atomically replaces files on disk
    /// - Cache invalidation ensures stale data isn't served
    /// - Ongoing queries may fail temporarily (acceptable for destructive operation)
    /// //TODO: What happens if queries are happening on .replica index, then we gonna corrupted index? Shitttt!!!!!
    /// // We can mitigate this by ensuring that restore operation is atomic (write to temp file then rename)
    /// // So queries always load the latest .replica file before executing, so even if a query starts while restore is happening, it will either get the old or new .replica file, but never a corrupted one.
    pub async fn restore_backup(
        &self,
        source: &str,
        database: &str,
        backup_filename: &str,
    ) -> Result<()> {
        let db_key = format!("{}:{}", source, database);

        // Check if backup is running (avoid backup/restore conflict)
        if self.state.is_backup_running(&db_key).await {
            anyhow::bail!("Cannot restore while backup is in progress");
        }

        // Check if write is in progress (avoid write/restore conflict)
        // This is important because writes update both .bin and .replica
        if Self::is_write_in_progress(source, database).await {
            anyhow::bail!("Cannot restore while write operation is in progress");
        }

        let backup_path = self
            .config
            .backup_base_dir
            .join(source)
            .join(database)
            .join(backup_filename);

        if !backup_path.exists() {
            anyhow::bail!("Backup file not found: {}", backup_filename);
        }

        // Find database path
        let database_path = search_database_on_disk(database, source)
            .await
            .context("Database not found on disk")?;

        info!(
            "Starting restore for {}:{} from backup {}",
            source, database, backup_filename
        );

        // Invalidate cache BEFORE restore to prevent serving stale data
        // But this isnt needed checksum check always ensures we have latest index in memory,
        // If we dont have it, we load from disk before query, so we should be safe without explicit cache invalidation here
        // {
        //     let mut cache = crate::server::controller::INDEX_CACHE.write().await;
        //     cache.pop(&db_key);
        //     info!("Invalidated cache entry for {}", db_key);
        // }

        // Restore the backup (destructive - overwrites current files)
        // This atomically replaces .replica and metadata.json
        // Ongoing queries may fail with "file not found" - this is acceptable
        // for a destructive operation. Next query will reload from restored files.
        restore_database_backup(&backup_path, &database_path)
            .await
            .context("Failed to restore backup")?;

        // Reload metadata from restored files
        let metadata_path = database_path.join("metadata.json");
        if metadata_path.exists() {
            match read_embeddings_metadata(&database_path).await {
                Ok(metadata) => {
                    // Update node count in SERVER_FILE
                    let mut server_file = SERVER_FILE.write().await;
                    if let Err(e) = server_file.update_node_count(
                        source,
                        database,
                        metadata.total_vectors as u32,
                    ) {
                        warn!("Failed to update node count after restore: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Failed to read metadata after restore: {}", e);
                }
            }
        }

        info!(
            "Restore completed for {}:{} from {}",
            source, database, backup_filename
        );

        Ok(())
    }

    /// Delete a specific backup file
    pub async fn delete_backup(
        &self,
        source: &str,
        database: &str,
        backup_filename: &str,
    ) -> Result<()> {
        let backup_path = self
            .config
            .backup_base_dir
            .join(source)
            .join(database)
            .join(backup_filename);

        if !backup_path.exists() {
            anyhow::bail!("Backup file not found: {}", backup_filename);
        }

        delete_backup_file(&backup_path)
            .await
            .context("Failed to delete backup file")?;

        info!(
            "Deleted backup {} for {}:{}",
            backup_filename, source, database
        );

        Ok(())
    }
}

impl Clone for BackupState {
    fn clone(&self) -> Self {
        Self {
            ongoing_backups: Arc::clone(&self.ongoing_backups),
        }
    }
}
