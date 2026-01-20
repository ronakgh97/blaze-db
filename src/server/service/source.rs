use crate::core::{ServerConfig, get_source_path, save_config};
use crate::server::dto::{CreateSourceRequest, CreateSourceResponse, ListResponse};
use anyhow::Result;

pub async fn create_new_source(request: CreateSourceRequest) -> Result<CreateSourceResponse> {
    let src_id = uuid::Uuid::new_v4().to_string(); // TODO: Use src_id meaningfully later
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let source_name = request.source_name;

    let mut config =
        ServerConfig::load_config(&ServerConfig::get_default_server_config_path()?).await?;

    // Update the server file accordingly
    config.data_source.add_source(source_name.clone())?;

    config.data_source.create_source_dir().await?;

    save_config(ServerConfig::get_default_server_config_path()?, &config).await?;

    Ok(CreateSourceResponse {
        id: src_id,
        source: source_name,
        created_at: timestamp,
    })
}

// List all sources and their databases from the disk (server file)
pub async fn list_source() -> Result<Vec<ListResponse>> {
    let config =
        ServerConfig::load_config(&ServerConfig::get_default_server_config_path()?).await?;
    let sources = config.data_source.source_name.unwrap_or_default();

    let mut response: Vec<ListResponse> = Vec::new();

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
