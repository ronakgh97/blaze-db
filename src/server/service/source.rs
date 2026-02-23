use crate::core::{SERVER_FILE, Source};
use crate::server::VectorBaseObject;
use crate::server::controller::ErrorTypes;
use crate::server::dto::{CreateSourceRequest, CreateSourceResponse, ListResponse};
use anyhow::Result;

pub async fn create_new_source(request: CreateSourceRequest) -> Result<CreateSourceResponse> {
    let src_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let source_name = request.source_name;

    if let Some(backup_interval) = request.backup_interval_hours
        && backup_interval < -1
    {
        return Err(ErrorTypes::InvalidField(format!(
                "backup_interval_hours must be -1 (disabled), 0 (default), or a positive integer. Got {}",
                backup_interval
            ))
            .into());
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

/// Lists all sources and their vector bases (databases), from SERVER_FILE in-memory state. Does not read from disk.
/// Use minimal READ Lock on SERVER_FILE, then release before any heavy processing or I/O.
pub async fn list_source() -> Result<Vec<ListResponse>> {
    // Get all source_obj and their respective vector base infos
    let source_obj: Vec<Source> = {
        let server_file = SERVER_FILE.read().await;
        server_file.get_all_sources()?
    };

    let mut response: Vec<ListResponse> = Vec::with_capacity(source_obj.len());

    for source in source_obj {
        let source_name = source.source_name.clone();
        let mut vector_bases: Vec<VectorBaseObject> = Vec::with_capacity(source.vector_bases.len());

        for db in &source.vector_bases {
            let vector_obj = VectorBaseObject {
                id: db.vb_id.clone(),
                name: db.vb_name.clone(),
                dimensions: db.dimension as usize,
                node_count: db.node_count as usize,
                metrics: db.metric_type.clone(),
                created_at: db.created_at.clone(),
            };

            vector_bases.push(vector_obj);
        }
        response.push(ListResponse {
            from_sources: source_name,
            databases: vector_bases,
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
