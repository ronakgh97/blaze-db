use crate::core::{ClientConfig, SERVER_FILE, get_source_path, save_config};
use anyhow::Result;

// TODO: FIX 'if' statements
pub async fn init_run_server() -> Result<()> {
    println!("Initializing dotfiles/src...");

    // Check if default source already exists to avoid overwriting
    let default_src_path = get_source_path()?.join("default_src");

    {
        let server_file = SERVER_FILE.read().await;

        if default_src_path.exists() && server_file.source_exists("default_src")? {
            println!("Defaults already initialized");
            return Ok(());
        }
    }

    // Create default source if it doesn't exist
    let mut server_file = SERVER_FILE.write().await;

    let src_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    if !server_file.source_exists("default_src")? {
        server_file
            .add_source(src_id, "default_src".to_string(), timestamp)
            .await?;
        println!(" Created default source");
    }

    println!(" Server defaults initialized");

    Ok(())
}

pub async fn init_run_client(url: Option<String>) -> Result<()> {
    println!("Initializing dotfiles...");

    let mut config = ClientConfig::default();
    if let Some(url) = url {
        config.update(url, config.timeout);
    }

    save_config(ClientConfig::get_default_user_config_path()?, &config).await?;

    println!(" Client initialized with URL: {}", config.url);

    Ok(())
}
