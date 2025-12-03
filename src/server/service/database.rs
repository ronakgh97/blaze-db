use crate::core::get_source_path;
use crate::server::get_active_source;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use anyhow::Result;
use chrono::{Utc};
use uuid::Uuid;

pub async fn create_new_database(request: CreateDatabaseRequest) -> Result<CreateDatabaseResponse> {
    let database_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let source_path = get_source_path()?;
    let active_source = get_active_source().unwrap();

    let name = request.name;
    let dimensions = &request.dimensions;

    let file_name = format!("#{}_#{}_#{}_#{}", name, database_id, dimensions, timestamp);

    let database_path = source_path.join(active_source).join(&file_name);
    tokio::fs::create_dir_all(database_path).await?;

    Ok(CreateDatabaseResponse {
        id: database_id,
        name,
    })
}
