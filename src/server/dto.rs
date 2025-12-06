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
    pub total_lines: usize,
}
