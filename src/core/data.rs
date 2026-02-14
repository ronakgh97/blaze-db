use crate::core::Metrics;
use serde::{Deserialize, Serialize};

/// Represents a data source containing multiple vector databases
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    pub src_id: String,
    pub source_name: String,
    pub vector_bases: Vec<VectorBase>,
    pub created_at: String,
    /// Backup interval in hours (0 = use default from global config, -1 = disabled, >0 = custom interval)
    pub backup_interval_hours: i32,
}

/// Represents a vector database within a source
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VectorBase {
    pub vb_id: String,
    pub vb_name: String,
    pub dimension: u32,
    pub node_count: u32,
    pub created_at: String,
    pub last_queried_at: String,
    pub metric_type: Metrics,
    /// (-1 = disabled, 0 = use source interval, >0 = custom interval in hours)
    pub backup_interval_hours: i32,
    /// When the next backup is scheduled (ISO 8601 timestamp)
    pub will_backup_at: Option<String>,
    /// When the last backup completed successfully (ISO 8601 timestamp)
    pub last_backup_at: Option<String>,
}

impl Default for Source {
    fn default() -> Self {
        let uuid = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            src_id: uuid,
            source_name: "default_src".to_string(),
            vector_bases: vec![],
            created_at: timestamp,
            backup_interval_hours: 0,
        }
    }
}

impl Source {
    /// Create a new source with ID and timestamp
    pub fn new(src_id: String, source_name: String, created_at: String) -> Self {
        Self {
            src_id,
            source_name,
            vector_bases: vec![],
            created_at,
            backup_interval_hours: 0,
        }
    }

    /// Create a new source with generated ID and current timestamp
    pub fn new_with_generated(source_name: String) -> Self {
        let uuid = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            src_id: uuid,
            source_name,
            vector_bases: vec![],
            created_at: timestamp,
            backup_interval_hours: 0,
        }
    }

    // /// Calculate when the next backup should occur based on interval
    // pub fn schedule_next_backup(&mut self, interval_hours: u32) {
    //     if interval_hours == 0 {
    //         self.will_backup_at = None;
    //         return;
    //     }
    //     let next_backup = chrono::Utc::now() + chrono::Duration::hours(interval_hours as i64);
    //     self.will_backup_at = Some(next_backup.to_rfc3339());
    // }
    //
    // /// Check if backup is due (will_backup_at <= now)
    // pub fn is_backup_due(&self) -> bool {
    //     match &self.will_backup_at {
    //         None => false,
    //         Some(timestamp) => match chrono::DateTime::parse_from_rfc3339(timestamp) {
    //             Ok(backup_time) => chrono::Utc::now() >= backup_time.with_timezone(&chrono::Utc),
    //             Err(_) => false,
    //         },
    //     }
    // }

    /// Add a vector base to this source
    pub fn add_vector_base(&mut self, vb: VectorBase) {
        self.vector_bases.push(vb);
    }

    /// Remove a vector base by ID
    pub fn remove_vector_base(&mut self, vb_id: &str) -> Option<VectorBase> {
        if let Some(index) = self.vector_bases.iter().position(|vb| vb.vb_id == vb_id) {
            Some(self.vector_bases.remove(index))
        } else {
            None
        }
    }

    /// Find a vector base by name
    pub fn find_vector_base(&self, vb_name: &str) -> Option<&VectorBase> {
        self.vector_bases.iter().find(|vb| vb.vb_name == vb_name)
    }

    /// Find a vector base by name (mutable)
    pub fn find_vector_base_mut(&mut self, vb_name: &str) -> Option<&mut VectorBase> {
        self.vector_bases
            .iter_mut()
            .find(|vb| vb.vb_name == vb_name)
    }

    /// Update a vector base
    pub fn update_vector_base(&mut self, updated_vb: VectorBase) -> bool {
        if let Some(vb) = self
            .vector_bases
            .iter_mut()
            .find(|vb| vb.vb_id == updated_vb.vb_id)
        {
            *vb = updated_vb;
            true
        } else {
            false
        }
    }
}

impl Default for VectorBase {
    fn default() -> Self {
        let uuid = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            vb_id: uuid,
            vb_name: "default_vb".to_string(),
            dimension: 1024,
            node_count: 0,
            created_at: timestamp.clone(),
            last_queried_at: timestamp,
            metric_type: Metrics::Cosine,
            backup_interval_hours: 0,
            will_backup_at: None,
            last_backup_at: None,
        }
    }
}

impl VectorBase {
    /// Create a new vector base with generated ID and timestamp
    pub fn new(vb_name: String, dimension: u32, metric_type: Metrics) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            vb_id: uuid::Uuid::new_v4().to_string(),
            vb_name,
            dimension,
            node_count: 0,
            created_at: timestamp.clone(),
            last_queried_at: timestamp,
            metric_type,
            backup_interval_hours: 0,
            will_backup_at: None,
            last_backup_at: None,
        }
    }

    /// Calculate when the next backup should occur based on interval
    pub fn schedule_next_backup(&mut self, interval_hours: u32) {
        if interval_hours == 0 {
            self.will_backup_at = None;
            return;
        }
        let next_backup = chrono::Utc::now() + chrono::Duration::hours(interval_hours as i64);
        self.will_backup_at = Some(next_backup.to_rfc3339());
    }

    /// Check if backup is due (will_backup_at <= now)
    pub fn is_backup_due(&self) -> bool {
        match &self.will_backup_at {
            None => false,
            Some(timestamp) => match chrono::DateTime::parse_from_rfc3339(timestamp) {
                Ok(backup_time) => chrono::Utc::now() >= backup_time.with_timezone(&chrono::Utc),
                Err(_) => false,
            },
        }
    }

    /// Update the last accessed timestamp
    pub fn touch(&mut self) {
        self.last_queried_at = chrono::Utc::now().to_rfc3339();
    }

    /// Update the node count
    pub fn set_node_count(&mut self, count: u32) {
        self.node_count = count;
        self.touch();
    }
}
