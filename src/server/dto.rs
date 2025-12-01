use serde::{Deserialize, Serialize};

/// Request DTO for database creation
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub description: Option<String>,
    pub dimensions: Option<usize>,
}

/// Response DTO for database creation
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateDatabaseResponse {
    pub id: String,
    pub name: String,
}
