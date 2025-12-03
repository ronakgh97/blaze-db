use serde::{Deserialize, Serialize};

/// Response DTO for health check
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HealthCheckResponse {
    pub status: String,
    pub service: String,
    pub uptime_hrs: u64,
}

#[allow(unused)]
/// Request DTO for database creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub dimensions: usize,
}

#[allow(unused)]
/// Response DTO for database creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDatabaseResponse {
    pub id: String,
    pub name: String,
}
