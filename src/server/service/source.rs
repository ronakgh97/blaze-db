use crate::core::{SERVER_FILE, get_source_path};
use crate::server::controller::ErrorTypes;
use crate::server::dto::{CreateSourceRequest, CreateSourceResponse, ListResponse};
use anyhow::Result;

pub async fn create_new_source(request: CreateSourceRequest) -> Result<CreateSourceResponse> {
    let src_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let source_name = request.source_name;

    if let Some(backup_interval) = request.backup_interval_hours {
        if backup_interval < -1 {
            return Err(ErrorTypes::InvalidField(format!(
                "backup_interval_hours must be -1 (disabled), 0 (default), or a positive integer. Got {}",
                backup_interval
            ))
            .into());
        }
    }

    let backup_interval_hours = request.backup_interval_hours.unwrap_or(0); // 0 means use default from config

    // TODO: LOCK CONTENTION - Write lock held during directory creation (async I/O)
    // Similar issue as create_new_database - blocks all other operations
    // The add_source() method creates directory on disk while holding this lock
    let mut server_file = SERVER_FILE.write().await;

    // Check if source already exists
    if server_file.source_exists(&source_name)? {
        return Err(ErrorTypes::SourceAlreadyExists(format!(
            "Source '{}' already exists",
            source_name
        ))
        .into());
    }

    // Add new source (creates directory automatically)
    let mut source = server_file
        .add_source(src_id.clone(), source_name.clone(), timestamp.clone())
        .await?;

    // Set backup interval if provided
    source.backup_interval_hours = backup_interval_hours;
    server_file.update_source(source.clone())?;

    Ok(CreateSourceResponse {
        id: src_id,
        source: source.source_name,
        created_at: timestamp,
    })
}

// TODO: PERFORMANCE - Still scanning the disk for databases, need to optimize by storing in server file
// This function does filesystem I/O while holding SERVER_FILE read lock
// Impact: Blocks all write operations (create_source, add_database, etc.)

// List all sources and their databases from the disk (server file)
pub async fn list_source() -> Result<Vec<ListResponse>> {
    let server_file = SERVER_FILE.read().await;

    // List all sources from server file
    let sources = server_file.list_sources()?;

    let mut response: Vec<ListResponse> = Vec::new();

    // Listing databases for each source
    for src in sources {
        let mut databases: Vec<String> = Vec::new();
        let source_path = get_source_path()?.join(&src);

        if source_path.exists() {
            let mut entries = tokio::fs::read_dir(source_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    databases.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        response.push(ListResponse {
            from_sources: src,
            databases,
        });
    }

    Ok(response)
}

#[allow(unused)]
pub async fn load_indexes() -> Result<()> {
    unimplemented!("This function is not yet implemented");
}

#[allow(unused)]
pub async fn unload_indexes() -> Result<()> {
    unimplemented!("This function is not yet implemented");
}
