use crate::core::Metrics;
use serde::{Deserialize, Serialize};

/// Response DTO for health check
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HealthCheckResponse {
    pub status: String,
    pub service: String,
    pub uptime_hrs: f32,
}

/// Request DTO for database creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub source: String,
    pub metrics: Option<Metrics>, // COSINE, EUCLIDEAN, DOT_PRODUCT, etc. Default to COSINE if not provided
    pub dimensions: usize,
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
pub struct ListResponse {
    pub from_sources: String,
    pub databases: Vec<String>,
    // pub indexes: Vec<String>, //TODO: Maybe return index details later? like size, entries, etc.
}

/// Request DTO for embedding data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedRequest {
    pub batch_content: Vec<Vec<String>>,
    pub database: String,
    pub source: String,
    pub batch: usize,
}

/// Response DTO for embedding data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedResponse {
    pub database: String,
    pub source: String,
    pub total_entries: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VectorDataDto {
    pub embedding: Vec<f32>,
    pub metadata: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InsertRequest {
    pub vectors: Vec<VectorDataDto>, // TODO: Change to batch insert later
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
    pub chunk: String,
    pub score: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateSourceRequest {
    pub source_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateSourceResponse {
    pub id: String,
    pub source: String,
    pub created_at: String,
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
