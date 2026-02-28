use crate::core::Metrics;
use serde::{Deserialize, Serialize};

/// Response DTO for health check
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HealthCheckResponse {
    pub status: String,
    pub service: String,
    pub uptime_hrs: f64,
}

/// Request DTO for database creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub source: String,
    pub metrics: Option<Metrics>, // COSINE, EUCLIDEAN, DOT_PRODUCT, etc. Default to COSINE if not provided
    pub dimensions: usize,
    // None or Some(0) = use default backup interval from source/global config, Some(-1) = disabled, Some(x) = custom interval in hours
    pub backup_interval_hours: Option<i32>,
}

/// Response DTO for database creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDatabaseResponse {
    pub id: String,
    pub name: String,
    pub metrics: Metrics,
    pub dimensions: usize,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateSourceRequest {
    pub source_name: String,
    // None or Some(0) = use default backup interval from global config, Some(-1) = disabled, Some(x) = custom interval in hours
    pub backup_interval_hours: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateSourceResponse {
    pub id: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListResponse {
    pub from_sources: String,
    pub databases: Vec<VectorBaseObject>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VectorBaseObject {
    pub id: String,
    pub name: String,
    pub dimensions: usize,
    pub node_count: usize,
    pub metrics: Metrics,
    pub created_at: String,
}

/// Request DTO for embedding data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedRequest {
    pub batch_content: Vec<Vec<EmbedData>>,
    pub database: String,
    pub source: String,
    pub batch: usize,
}

/// DTO for individual data to be embedded with id
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedData {
    pub id: String,
    pub embed_data: String,
}

/// Response DTO for embedding data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedResponse {
    pub database: String,
    pub source: String,
    pub total_entries: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QueryRequest {
    pub query: String,
    pub database: String,
    pub source: String,
    pub top_k: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QueryResponse {
    pub results: Vec<QueryResult>,
    pub io_time_sec: f64,
    pub search_time_sec: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QueryResult {
    pub id: String,
    pub chunk: String,
    pub score: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InsertRequest {
    pub nodes: Vec<Vec<VectorDataDto>>, // batch of vectors with metadata, now batch index can be done
    pub database: String,
    pub source: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InsertResponse {
    pub database: String,
    pub source: String,
    pub total_inserted: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VectorDataDto {
    pub id: String,
    pub embedding: Vec<f32>,
    pub metadata: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VectorQueryRequest {
    pub query_vector: Vec<f32>,
    pub database: String,
    pub source: String,
    pub top_k: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VectorQueryResponse {
    pub results: Vec<VectorQueryResult>,
    pub io_time_sec: f64,
    pub search_time_sec: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VectorQueryResult {
    pub vectordata: VectorDataDto,
    pub score: f32,
}

// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct SourceLoadRequest {
//     pub source: String,
// }
//
// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct SourceLoadResponse {
//     pub source: String,
//     pub database: String,
//     pub total_index: usize,
// }
//
// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct SourceUnloadRequest {
//     pub source: String,
// }
//
// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct SourceUnloadResponse {
//     pub source: String,
//     pub database: String,
//     pub total_unloaded: usize,
// }

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetIndexDetailsRequest {
    pub source: String,
    pub database: String,
    pub page: usize,
    pub show_tombstone: bool, // whether to include deleted entries (tombstones) in the response
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetIndexDetailsResponse {
    pub source: String,
    pub database: String,
    pub total_pages: usize,
    pub current_page: usize,
    pub entries: Vec<VectorDataDto>,
}

/// Request DTO for creating a backup
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateBackupRequest {
    pub source: String,
    pub database: String,
}

/// Response DTO for backup creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateBackupResponse {
    pub success: bool,
    pub backup_info: Option<BackupInfoDto>,
    pub message: String,
}

/// Request DTO for listing backups
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListBackupsRequest {
    pub source: String,
    pub database: String,
}

/// Response DTO for listing backups
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListBackupsResponse {
    pub backups: Vec<BackupInfoDto>,
}

/// Request DTO for restoring from backup
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RestoreBackupRequest {
    pub source: String,
    pub database: String,
    pub backup_filename: String,
}

/// Response DTO for restore operation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RestoreBackupResponse {
    pub success: bool,
    pub message: String,
}

/// Request DTO for deleting a backup
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeleteBackupRequest {
    pub source: String,
    pub database: String,
    pub backup_filename: String,
}

/// Response DTO for delete backup operation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeleteBackupResponse {
    pub success: bool,
    pub message: String,
}

/// DTO for backup information
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BackupInfoDto {
    pub filename: String,
    pub timestamp: String,
    pub size_mb: f64,
    pub source: String,
    pub database: String,
}
