use crate::core::{get_source_path, load_config};
use crate::prelude::log;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use crate::server::{ListDatabasesResponse, get_active_source};
use crate::{info, warn};
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

pub async fn create_new_database(request: CreateDatabaseRequest) -> Result<CreateDatabaseResponse> {
    let database_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let source_path = get_source_path()?;
    let active_source = get_active_source().unwrap();

    let name = request.name;
    let dimensions = &request.dimensions;

    let file_name = format!("#{}_{}_#{}_#{}", name, database_id, dimensions, timestamp);

    let database_path = source_path.join(active_source).join(&file_name);
    info!("Creating database directory: {:?}", database_path);
    tokio::fs::create_dir_all(&database_path).await?;
    info!("Database '{}' initialized at: {:?}", name, database_path);

    Ok(CreateDatabaseResponse {
        id: database_id,
        name,
        source: active_source.to_string(),
        created_at: timestamp,
    })
}

/// List all databases from all source directories.
pub async fn list_databases() -> Result<Vec<ListDatabasesResponse>> {
    let config = load_config().await?;
    let base_path = get_source_path()?;
    let sources = config.data_source.source_name.unwrap_or_default();

    let mut result = Vec::new();

    for source in sources {
        let dir = base_path.join(&source);
        if !dir.exists() {
            warn!("Source directory does not exist: {:?}", dir);
            continue;
        }

        info!("Scanning source '{}' for databases", source);
        let mut databases = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if parse_database_name(&name).is_some() {
                databases.push(name);
            }
        }

        result.push(ListDatabasesResponse {
            from_sources: source,
            databases,
        });
    }

    Ok(result)
}

/// Search for a database by name in the active source directory.
pub async fn search_database(name: String) -> Result<PathBuf> {
    info!("Searching for database '{}'", name);
    let source_path = get_source_path()?;
    let active_source = get_active_source().unwrap();
    let dir_path = source_path.join(active_source);

    let mut read_dir = tokio::fs::read_dir(&dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        if let Some((db_name, _, _, _)) = parse_database_name(&file_name)
            && db_name == name
        {
            info!("Database '{}' found at: {:?}", name, entry.path());
            return Ok(entry.path());
        }
    }

    anyhow::bail!("Database with name '{}' not found", name);
}

/// Parse the database name from the given filename.
/// The return format is: (name,id,dimensions,timestamp)
/// Returns None if the format is incorrect.
/// Expected format: #name_#id_#dimensions_#timestamp
fn parse_database_name(file_name: &str) -> Option<(String, String, String, String)> {
    let parts: Vec<_> = file_name.split("_#").collect();
    match parts[..] {
        [name, id, dimensions, timestamp] => {
            // Remove leading '#' from name
            let name = name.strip_prefix('#').unwrap_or(name);
            Some((
                name.to_owned(),
                id.to_owned(),
                dimensions.to_owned(),
                timestamp.to_owned(),
            ))
        }
        _ => None,
    }
}
