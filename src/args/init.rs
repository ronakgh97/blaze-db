use crate::core::{SERVER_FILE, UserConfig, get_default_backups_dir, get_source_path, save_config};
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
        let mut source = server_file
            .add_source(src_id, "default_src".to_string(), timestamp)
            .await?;

        // Set backup interval to 0, which uses default backup interval from config (currently 24hrs)
        source.backup_interval_hours = 0;
        server_file.update_source(source.clone())?;

        println!(" Created default source");
    }

    // Create backups dir if it doesn't exist
    let backup_dir = get_default_backups_dir()?;
    if !backup_dir.exists() {
        tokio::fs::create_dir_all(&backup_dir).await?;
        println!(" Created backups directory");
    }

    Ok(())
}

pub async fn init_run_client() -> Result<()> {
    println!("Initializing dotfiles...");

    let config = UserConfig::default();

    save_config(UserConfig::get_default_path()?, &config).await?;

    println!(" Client defaults initialized");
    Ok(())
}
