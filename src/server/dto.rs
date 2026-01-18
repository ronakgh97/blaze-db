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
    pub dimensions: usize,
}

/// Response DTO for database creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDatabaseResponse {
    pub id: String,
    pub name: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListDatabasesResponse {
    pub from_sources: String,
    pub databases: Vec<String>,
}

/// Request DTO for embedding data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedRequest {
    pub file_content: Vec<Vec<String>>,
    pub database: String,
    pub batch: usize,
}

/// Response DTO for embedding data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmbedResponse {
    pub database: String,
    pub total_entries: usize,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QueryRequest {
    pub query: String,
    pub database: String,
    pub top_k: usize,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QueryResponse {
    pub results: Vec<QueryResult>,
    pub time_ms: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QueryResult {
    pub chunk: String,
    pub score: f32,
}

// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct CreateSourceRequest {
//     pub source_name: String,
// }
//
// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct CreateSourceResponse {
//     pub id: String,
//     pub source: String,
//     pub created_at: String,
// }
//
// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct ListSourcesResponse {
//     pub sources: Vec<String>,
// }

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
