use serde::{Deserialize, Serialize};

/// Response DTO for health check
#[derive(Debug, Deserialize, Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub service: String,
    pub uptime: u128,
}

/// Request DTO for database creation
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub dimensions: usize,
}

/// Response DTO for database creation
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateDatabaseResponse {
    pub id: String,
    pub name: String,
}
