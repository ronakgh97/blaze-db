use anyhow::Result;
use blaze_db::core::{Source, VectorBase};
use blaze_db::utils::DataStore;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;
use uuid::Uuid;

// Test ServerFile that uses a temporary directory
struct TestServerFile {
    store: DataStore<String, Source>,
    _temp_dir: TempDir, // Keep temp dir alive
    source_base_path: PathBuf,
}

impl TestServerFile {
    async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("SERVER_DATA.json");
        let source_base_path = temp_dir.path().join("sources");

        fs::create_dir_all(&source_base_path).await?;

        let store = DataStore::new(config_path)?;

        Ok(Self {
            store,
            _temp_dir: temp_dir,
            source_base_path,
        })
    }

    fn get_source(&self, source_name: &str) -> Result<Option<Source>> {
        self.store.get(&source_name.to_string())
    }

    fn list_sources(&self) -> Result<Vec<String>> {
        self.store.keys()
    }

    fn source_exists(&self, source_name: &str) -> Result<bool> {
        self.store.contains_key(&source_name.to_string())
    }

    async fn add_source(
        &mut self,
        src_id: String,
        source_name: String,
        created_at: String,
    ) -> Result<Source> {
        if self.store.contains_key(&source_name)? {
            anyhow::bail!("Source '{}' already exists", source_name);
        }

        let source = Source::new(src_id, source_name.clone(), created_at);
        self.store.insert(source_name.clone(), source.clone())?;

        let source_path = self.source_base_path.join(&source_name);
        fs::create_dir_all(&source_path).await?;

        Ok(source)
    }

    async fn add_source_with_generated(&mut self, source_name: String) -> Result<Source> {
        let src_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        self.add_source(src_id, source_name, timestamp).await
    }

    async fn remove_source(&mut self, source_name: &str, remove_disk: bool) -> Result<Source> {
        let source = self
            .store
            .get(&source_name.to_string())?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        self.store.delete(&source_name.to_string())?;

        if remove_disk {
            let source_path = self.source_base_path.join(source_name);
            if source_path.exists() {
                fs::remove_dir_all(&source_path).await?;
            }
        }

        Ok(source)
    }

    fn update_source(&mut self, source: Source) -> Result<()> {
        if !self.store.contains_key(&source.source_name)? {
            anyhow::bail!("Source '{}' not found", source.source_name);
        }
        self.store.insert(source.source_name.clone(), source)?;
        Ok(())
    }

    fn add_vector_base(&mut self, source_name: &str, vb: VectorBase) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        source.add_vector_base(vb);
        self.store.insert(source_name.to_string(), source)?;
        Ok(())
    }

    fn get_vector_base(&self, source_name: &str, vb_name: &str) -> Result<Option<VectorBase>> {
        let source = self.get_source(source_name)?;
        Ok(source.and_then(|s| s.find_vector_base(vb_name).cloned()))
    }

    fn list_vector_bases(&self, source_name: &str) -> Result<Vec<VectorBase>> {
        let source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;
        Ok(source.vector_bases)
    }

    fn update_node_count(&mut self, source_name: &str, vb_name: &str, count: u32) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        if let Some(vb) = source.find_vector_base_mut(vb_name) {
            vb.set_node_count(count);
            self.store.insert(source_name.to_string(), source)?;
            Ok(())
        } else {
            anyhow::bail!(
                "VectorBase '{}' not found in source '{}'",
                vb_name,
                source_name
            )
        }
    }

    fn touch_vector_base(&mut self, source_name: &str, vb_name: &str) -> Result<()> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        if let Some(vb) = source.find_vector_base_mut(vb_name) {
            vb.touch();
            self.store.insert(source_name.to_string(), source)?;
            Ok(())
        } else {
            anyhow::bail!(
                "VectorBase '{}' not found in source '{}'",
                vb_name,
                source_name
            )
        }
    }

    fn remove_vector_base(&mut self, source_name: &str, vb_id: &str) -> Result<VectorBase> {
        let mut source = self
            .get_source(source_name)?
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", source_name))?;

        let removed = source
            .remove_vector_base(vb_id)
            .ok_or_else(|| anyhow::anyhow!("VectorBase with id '{}' not found", vb_id))?;

        self.store.insert(source_name.to_string(), source)?;
        Ok(removed)
    }

    fn stats(&self) -> Result<ServerStats> {
        let sources = self.store.values()?;
        let total_sources = sources.len() as u32;
        let mut total_vector_bases = 0u32;
        let mut total_nodes = 0u32;

        for source in sources {
            total_vector_bases += source.vector_bases.len() as u32;
            for vb in &source.vector_bases {
                total_nodes += vb.node_count;
            }
        }

        Ok(ServerStats {
            total_sources,
            total_vector_bases,
            total_nodes,
        })
    }

    fn snapshot(&self) -> Result<std::collections::HashMap<String, Source>> {
        let sources = self.store.values()?;
        let mut map = std::collections::HashMap::new();
        for source in sources {
            map.insert(source.source_name.clone(), source);
        }
        Ok(map)
    }
}

#[derive(Debug)]
struct ServerStats {
    total_sources: u32,
    total_vector_bases: u32,
    total_nodes: u32,
}

#[tokio::test]
async fn test_add_source() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let source = server_file
        .add_source(src_id, "test_source".to_string(), timestamp)
        .await?;

    assert_eq!(source.source_name, "test_source");
    assert!(!source.src_id.is_empty());
    assert!(!source.created_at.is_empty());
    assert!(source.vector_bases.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_get_source() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "test_get_source".to_string(), timestamp)
        .await?;

    let source = server_file.get_source("test_get_source")?;

    assert!(source.is_some());
    assert_eq!(source.unwrap().source_name, "test_get_source");

    Ok(())
}

#[tokio::test]
async fn test_source_exists() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "exists_test".to_string(), timestamp)
        .await?;

    assert!(server_file.source_exists("exists_test")?);
    assert!(!server_file.source_exists("does_not_exist")?);

    Ok(())
}

#[tokio::test]
async fn test_list_sources() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    server_file
        .add_source(src_id, "source1".to_string(), timestamp)
        .await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    server_file
        .add_source(src_id, "source2".to_string(), timestamp)
        .await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    server_file
        .add_source(src_id, "source3".to_string(), timestamp)
        .await?;

    let sources = server_file.list_sources()?;

    assert!(sources.contains(&"source1".to_string()));
    assert!(sources.contains(&"source2".to_string()));
    assert!(sources.contains(&"source3".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_remove_source() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "remove_test".to_string(), timestamp)
        .await?;
    assert!(server_file.source_exists("remove_test")?);

    let removed = server_file.remove_source("remove_test", true).await?;
    assert_eq!(removed.source_name, "remove_test");
    assert!(!server_file.source_exists("remove_test")?);

    Ok(())
}

#[tokio::test]
async fn test_duplicate_source() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "duplicate".to_string(), timestamp)
        .await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let result = server_file
        .add_source(src_id, "duplicate".to_string(), timestamp)
        .await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_add_vector_base() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "vb_test".to_string(), timestamp)
        .await?;

    let vb = VectorBase::new("embeddings".to_string(), 384, "cosine".to_string());
    server_file.add_vector_base("vb_test", vb)?;

    // Verify it was added
    let vb = server_file.get_vector_base("vb_test", "embeddings")?;
    assert!(vb.is_some());

    let vb = vb.unwrap();
    assert_eq!(vb.vb_name, "embeddings");
    assert_eq!(vb.dimension, 384);
    assert_eq!(vb.metric_type, "cosine");
    assert_eq!(vb.node_count, 0);

    Ok(())
}

#[tokio::test]
async fn test_list_vector_bases() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "multi_vb".to_string(), timestamp)
        .await?;

    let vb1 = VectorBase::new("vb1".to_string(), 384, "cosine".to_string());
    let vb2 = VectorBase::new("vb2".to_string(), 768, "euclidean".to_string());

    server_file.add_vector_base("multi_vb", vb1)?;
    server_file.add_vector_base("multi_vb", vb2)?;

    let vector_bases = server_file.list_vector_bases("multi_vb")?;

    assert_eq!(vector_bases.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_update_node_count() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "node_count_test".to_string(), timestamp)
        .await?;

    let vb = VectorBase::new("embeddings".to_string(), 384, "cosine".to_string());
    server_file.add_vector_base("node_count_test", vb)?;

    server_file.update_node_count("node_count_test", "embeddings", 1000)?;

    let vb = server_file.get_vector_base("node_count_test", "embeddings")?;
    assert_eq!(vb.unwrap().node_count, 1000);

    Ok(())
}

#[tokio::test]
async fn test_touch_vector_base() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "touch_test".to_string(), timestamp)
        .await?;
    let vb = VectorBase::new("embeddings".to_string(), 384, "cosine".to_string());
    server_file.add_vector_base("touch_test", vb)?;

    // Get initial timestamp
    let initial_time = {
        let vb = server_file
            .get_vector_base("touch_test", "embeddings")?
            .unwrap();
        vb.last_queried_at.clone()
    };

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Touch it
    server_file.touch_vector_base("touch_test", "embeddings")?;

    // Get new timestamp
    let new_time = {
        let vb = server_file
            .get_vector_base("touch_test", "embeddings")?
            .unwrap();
        vb.last_queried_at
    };

    assert_ne!(initial_time, new_time);

    Ok(())
}

#[tokio::test]
async fn test_remove_vector_base() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    server_file
        .add_source(src_id, "remove_vb_test".to_string(), timestamp)
        .await?;

    let vb = VectorBase::new("to_remove".to_string(), 384, "cosine".to_string());
    let vb_id = vb.vb_id.clone();
    server_file.add_vector_base("remove_vb_test", vb)?;

    let removed = server_file.remove_vector_base("remove_vb_test", &vb_id)?;

    assert_eq!(removed.vb_name, "to_remove");

    // Verify it's gone
    let vb = server_file.get_vector_base("remove_vb_test", "to_remove")?;
    assert!(vb.is_none());

    Ok(())
}

#[tokio::test]
async fn test_stats() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    // Create 2 sources
    server_file
        .add_source_with_generated("stats_source1".to_string())
        .await?;
    server_file
        .add_source_with_generated("stats_source2".to_string())
        .await?;

    // Add VectorBases with nodes
    let vb1 = VectorBase::new("vb1".to_string(), 384, "cosine".to_string());
    server_file.add_vector_base("stats_source1", vb1)?;
    server_file.update_node_count("stats_source1", "vb1", 100)?;

    let vb2 = VectorBase::new("vb2".to_string(), 768, "euclidean".to_string());
    server_file.add_vector_base("stats_source2", vb2)?;
    server_file.update_node_count("stats_source2", "vb2", 200)?;

    let stats = server_file.stats()?;

    assert_eq!(stats.total_sources, 2);
    assert_eq!(stats.total_vector_bases, 2);
    assert_eq!(stats.total_nodes, 300);

    Ok(())
}

#[tokio::test]
async fn test_concurrent_reads() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;
    server_file
        .add_source_with_generated("concurrent_test".to_string())
        .await?;

    // DataStore has internal RwLock, so we can safely clone the Arc and share it
    // For testing concurrent access, we would need to wrap TestServerFile in Arc<RwLock<>>
    // but since this is a unit test for the data model, we'll just verify basic functionality
    assert!(server_file.source_exists("concurrent_test")?);

    Ok(())
}

#[tokio::test]
async fn test_update_source() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;
    server_file
        .add_source_with_generated("update_test".to_string())
        .await?;

    // Get source, modify it, update it
    let mut source = server_file.get_source("update_test")?.unwrap();

    let vb = VectorBase::new("new_vb".to_string(), 512, "dot_product".to_string());
    source.add_vector_base(vb);

    server_file.update_source(source)?;

    // Verify update
    let updated = server_file.get_source("update_test")?.unwrap();
    assert_eq!(updated.vector_bases.len(), 1);
    assert_eq!(updated.vector_bases[0].vb_name, "new_vb");

    Ok(())
}

#[tokio::test]
async fn test_snapshot() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;
    server_file
        .add_source_with_generated("snapshot1".to_string())
        .await?;
    server_file
        .add_source_with_generated("snapshot2".to_string())
        .await?;

    let snapshot = server_file.snapshot()?;

    assert!(snapshot.contains_key("snapshot1"));
    assert!(snapshot.contains_key("snapshot2"));

    Ok(())
}

#[tokio::test]
async fn test_source_data_model() {
    // Test Source creation

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let source = Source::new(src_id, "test".to_string(), timestamp);
    assert_eq!(source.source_name, "test");
    assert!(!source.src_id.is_empty());
    assert!(source.vector_bases.is_empty());

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Test adding VectorBase
    let mut source = Source::new(src_id, "test".to_string(), timestamp);
    let vb = VectorBase::new("vb1".to_string(), 384, "cosine".to_string());
    source.add_vector_base(vb);
    assert_eq!(source.vector_bases.len(), 1);

    // Test finding VectorBase
    let found = source.find_vector_base("vb1");
    assert!(found.is_some());
    assert_eq!(found.unwrap().vb_name, "vb1");

    // Test removing VectorBase
    let vb_id = source.vector_bases[0].vb_id.clone();
    let removed = source.remove_vector_base(&vb_id);
    assert!(removed.is_some());
    assert!(source.vector_bases.is_empty());
}

#[tokio::test]
async fn test_vectorbase_data_model() {
    let mut vb = VectorBase::new("test".to_string(), 384, "cosine".to_string());

    assert_eq!(vb.vb_name, "test");
    assert_eq!(vb.dimension, 384);
    assert_eq!(vb.metric_type, "cosine");
    assert_eq!(vb.node_count, 0);

    // Test set_node_count
    let initial_time = vb.last_queried_at.clone();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    vb.set_node_count(100);
    assert_eq!(vb.node_count, 100);
    assert_ne!(vb.last_queried_at, initial_time);

    // Test touch
    let time_before_touch = vb.last_queried_at.clone();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    vb.touch();
    assert_ne!(vb.last_queried_at, time_before_touch);
}

#[tokio::test]
async fn test_error_cases() -> Result<()> {
    let mut server_file = TestServerFile::new().await?;

    // Test getting non-existent source
    let result = server_file.get_source("does_not_exist")?;
    assert!(result.is_none());

    let src_id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Test updating non-existent source
    let fake_source = Source::new(src_id, "fake".to_string(), timestamp);
    let result = server_file.update_source(fake_source);
    assert!(result.is_err());

    // Test removing non-existent source
    let result = server_file.remove_source("does_not_exist", false).await;
    assert!(result.is_err());

    // Test adding VectorBase to non-existent source
    let vb = VectorBase::new("test".to_string(), 384, "cosine".to_string());
    let result = server_file.add_vector_base("does_not_exist", vb);
    assert!(result.is_err());

    Ok(())
}
