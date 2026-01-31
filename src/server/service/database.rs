use crate::core::{SERVER_FILE, VectorBase, get_source_path};
use crate::server::controller::ErrorTypes;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use crate::{info, warn};
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

// Create a new database directory in the specified source.
// TODO: LOCK CONTENTION - This function holds SERVER_FILE write lock for entire operation
// The lock covers: validation (with I/O), duplicate check, metadata update (disk write), directory creation
// Blocks ALL other database/source operations during this time
// Current approach: Simple, correct, thread-safe - just slower than optimal
pub async fn create_new_database(request: CreateDatabaseRequest) -> Result<CreateDatabaseResponse> {
    let database_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();
    let name = &request.name;
    let dimensions = &request.dimensions;
    let source = &request.source;
    let source_path = get_source_path()?;

    if name.contains('/') || name.contains('\\') || name.contains('.') {
        return Err(ErrorTypes::InvalidField(format!(
            "Database name '{}' contains invalid characters",
            name
        ))
        .into());
    }

    // TODO: PERFORMANCE - This write lock is held for too long (includes async I/O)
    // Trade-off: Current approach is simpler and avoids race conditions
    let mut server_file = SERVER_FILE.write().await;

    // TODO: BOTTLENECK - is_source_valid() does filesystem I/O while holding write lock
    // Check if the source is valid (call directly to avoid deadlock)
    if server_file.is_source_valid(source).await? {
        info!("Using provided source: {}", source);
    } else {
        return Err(ErrorTypes::SourceNotFound(format!("Source '{}' is not valid", source)).into());
    }

    // Check if duplicates exist from server_file and return error if so
    if server_file.get_vector_base(source, name)?.is_some() {
        return Err(ErrorTypes::DatabaseAlreadyExists(format!(
            "Database '{}' already exists in source '{}'",
            name, source
        ))
        .into());
    }

    let vb = VectorBase {
        vb_id: database_id.clone(),
        vb_name: name.to_string(),
        dimension: 0,
        node_count: 0,
        created_at: timestamp.clone(),
        last_queried_at: timestamp.clone(),
        metric_type: "cosine".to_string(),
    };

    // TODO: PERFORMANCE - This triggers immediate disk write (save_to_disk in DataStore)
    // Every database creation writes the entire SERVER_DATA.json to disk
    // Track the new database in server_file (this writes to disk via DataStore)
    server_file.add_vector_base(source, vb)?;

    // Check if a database with the same name already exists in this source
    // let existing_databases = list_databases(source).await?;
    // for existing_db in &existing_databases {
    //     if let Some((existing_name, _, _, _)) = parse_database_name(existing_db) {
    //         if existing_name == *name {
    //             warn!(
    //                 "Database '{}' already exists in source '{}' (found: {})",
    //                 name, source, existing_db
    //             );
    //             return Err(ErrorTypes::DatabaseAlreadyExists(format!(
    //                 "Database '{}' already exists in source '{}'",
    //                 name, source
    //             ))
    //             .into());
    //         }
    //     }
    // }

    //let file_name = format!("#{}_#{}_#{}_#{}", name, database_id, dimensions, timestamp);

    let file_name = format!("{}", name); // that's it bro....no need for fancy names

    let database_path = source_path.join(&source).join(&file_name);

    // // Check if database directory path already exists (should not happen with UUID)
    // if database_path.exists() {
    //     return Err(ErrorTypes::DatabaseAlreadyExists(format!(
    //         "Database directory already exists at: {:?}",
    //         database_path
    //     ))
    //     .into());
    // }

    info!("Creating database directory: {:?}", database_path);
    tokio::fs::create_dir_all(&database_path).await?;
    info!("Database '{}' created at: {:?}", name, database_path);
    // Use `parse_database_name` to verify the created database name

    // if let Some((parsed_name, _, _, _)) = parse_database_name(&file_name) {
    //     if parsed_name != *name {
    //         return Err(ErrorTypes::InvalidField(format!(
    //             "Parsed database name '{}' does not match requested name '{}'",
    //             parsed_name, name
    //         ))
    //         .into());
    //     }
    // } else {
    //     return Err(ErrorTypes::InvalidField(format!(
    //         "Failed to parse database name from '{}'",
    //         file_name
    //     ))
    //     .into());
    // }

    Ok(CreateDatabaseResponse {
        id: database_id,
        name: name.to_string(),
        dimensions: *dimensions,
        source: source.to_string(),
        created_at: timestamp,
    })
}

#[allow(unused)]
/// List all databases from a source tracked in the server file, returns a vector of VectorBase.
pub async fn list_database_from_server_file(source: &String) -> Result<Vec<VectorBase>> {
    let server_file = SERVER_FILE.read().await;
    let mut result: Vec<VectorBase> = Vec::new();

    let source = server_file.get_source(source)?;

    if let Some(source) = source {
        for vb in &source.vector_bases {
            result.push(vb.clone());
        }
    }

    Ok(result)
}

/// List all databases from a source directory.
/// Return a vector of database names found in the specified sources, or an empty vector if none are found.
pub async fn list_databases_from_disk(source: &String) -> Result<Vec<String>> {
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
        databases.push(name);

        // let name = entry.file_name().to_string_lossy().to_string();
        // if let Some((db_name, id, dims, timestamp)) = parse_database_name(&name) {
        //     info!(
        //         "Found database: name='{}', id='{}', dims='{}', timestamp='{}'",
        //         db_name, id, dims, timestamp
        //     );
        //     databases.push(name);
        // } else {
        //     warn!("Skipping invalid database directory: {}", name);
        // }
    }

    result.extend(databases);

    Ok(result)
}

/// Search (Scan) a database dir by name in the specified source directory.
pub async fn search_database_on_disk(db_name: &String, sources: &String) -> Result<PathBuf> {
    info!("Searching for database '{}'", db_name);
    let source_path = get_source_path()?;
    let dir_path = source_path.join(sources);

    let mut read_dir = tokio::fs::read_dir(&dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name().into_string().unwrap_or_default();
        if file_name == *db_name {
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
        databases.push(entry.path());
    }

    Ok(databases)
}

#[allow(unused)]
#[deprecated(since = "2026-01-30", note = "No longer needed")]
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
