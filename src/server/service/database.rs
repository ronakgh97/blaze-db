use crate::core::{get_source_path, load_config};
use crate::server::get_active_source;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
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

    let file_name = format!("#{}_#{}_#{}_#{}", name, database_id, dimensions, timestamp);

    let database_path = source_path.join(active_source).join(&file_name);
    tokio::fs::create_dir_all(database_path).await?;

    Ok(CreateDatabaseResponse {
        id: database_id,
        name,
        source: active_source.to_string(),
        created_at: timestamp,
    })
}

/// List all databases from specified source directory.
/// If no source is provided, return all sources with respective databases.
/// Return format: Vec of (name,id,source_from,timestamp)
pub async fn list_databases(
    source: Option<String>,
) -> Result<Vec<(String, String, String, String)>> {
    let config = load_config().await?;
    let source_path = get_source_path()?;

    let sources_to_scan: Vec<String> = match source {
        Some(s) => vec![s],
        None => config.data_source.source_name.unwrap_or_default(),
    };

    let mut databases = Vec::new();

    for source_name in sources_to_scan {
        let dir_path = source_path.join(&source_name);

        if !dir_path.exists() {
            continue;
        }

        let mut read_dir = tokio::fs::read_dir(&dir_path).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let file_name = entry.file_name().into_string().unwrap_or_default();
            if let Some((name, id, _dimensions, timestamp)) = parse_database_name(&file_name) {
                databases.push((name, id, source_name.clone(), timestamp));
            }
        }
    }

    Ok(databases)
}

/// Search for a database by name in the active source directory.
pub async fn search_database(name: String) -> Result<PathBuf> {
    let source_path = get_source_path()?;
    let active_source = get_active_source().unwrap();
    let dir_path = source_path.join(active_source);

    let mut read_dir = tokio::fs::read_dir(&dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        if let Some((db_name, _, _, _)) = parse_database_name(&file_name) {
            if db_name == name {
                return Ok(entry.path());
            }
        }
    }

    anyhow::bail!("Database with name '{}' not found", name);
}

/// Parse the database name from the given file name.
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
