use crate::core::{check_source_valid, get_source_path};
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use crate::{info, warn};
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

// Create a new database directory in the specified source.
pub async fn create_new_database(request: CreateDatabaseRequest) -> Result<CreateDatabaseResponse> {
    let database_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let name = request.name;
    let dimensions = &request.dimensions;
    let source = &request.source;
    let source_path = get_source_path()?;

    // Check if the source is valid
    if check_source_valid(source).await? {
        info!("Using provided source: {}", source);
    } else {
        return Err(anyhow::anyhow!("Source '{}' is not valid", source));
    }

    let file_name = format!("#{}_#{}_#{}_#{}", name, database_id, dimensions, timestamp);

    let database_path = source_path.join(source).join(&file_name);
    // TODO: Check if database already exists
    info!("Creating database directory: {:?}", database_path);
    tokio::fs::create_dir_all(&database_path).await?;
    info!("Database '{}' initialized at: {:?}", name, database_path);

    Ok(CreateDatabaseResponse {
        id: database_id,
        name,
        source: source.to_string(),
        created_at: timestamp,
    })
}

/// List all databases from a source directories.
/// Return a vector of database names found in the specified sources, or an empty vector if none are found.
pub async fn list_databases(source: String) -> Result<Vec<String>> {
    // TODO: Maybe return Option type
    let base_src_path = get_source_path()?;

    // // Check validate sources
    // for source in &sources {
    //     if check_source_valid(source).await? {
    //         info!("Using provided source: {}", source);
    //     } else {
    //         return Err(anyhow::anyhow!("Source '{}' is not valid", source)); // TODO: Maybe Skip invalid sources instead of erroring out
    //     }
    // }

    let mut result: Vec<String> = Vec::new();

    let dir = base_src_path.join(&source);
    if !dir.exists() {
        warn!("Source directory does not exist: {:?}", dir);
        return Ok(result);
    }

    info!("Scanning source '{}' for databases", source);
    let mut databases = Vec::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if !metadata.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if let Some((db_name, id, dims, timestamp)) = parse_database_name(&name) {
            info!(
                "Found database: name='{}', id='{}', dims='{}', timestamp='{}'",
                db_name, id, dims, timestamp
            );
            databases.push(name);
        } else {
            warn!("Skipping invalid database directory: {}", name);
        }
    }

    result.extend(databases);

    Ok(result)
}

/// Get a database by name in the specified source directory.
pub async fn search_database(db_name: String, sources: String) -> Result<PathBuf> {
    info!("Searching for database '{}'", db_name);
    let source_path = get_source_path()?;
    let dir_path = source_path.join(sources);

    let mut read_dir = tokio::fs::read_dir(&dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        if let Some((parsed_name, _, _, _)) = parse_database_name(&file_name)
            && parsed_name == db_name
        {
            info!("Database '{}' found at: {:?}", db_name, entry.path());
            return Ok(entry.path());
        }
    }

    anyhow::bail!("Database with name '{}' not found", db_name);
}

#[allow(unused)]
/// Get all databases_dir_path from a specific source directory.
pub async fn get_databases_path_from_source(src_name: String) -> Result<Vec<PathBuf>> {
    let source_path = get_source_path()?;
    let dir_path = source_path.join(&src_name);

    let mut databases = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        if let Some(_) = parse_database_name(&file_name) {
            databases.push(entry.path());
        }
    }

    Ok(databases)
}

/// Parse the database name from the given filename.
/// The return format is: (name,id,dimensions,timestamp)
/// Returns None if the format is incorrect.
/// Expected format: #name_#id_#dimensions_#timestamp
pub fn parse_database_name(file_name: &str) -> Option<(String, String, String, String)> {
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
