use crate::core::SERVER_FILE;
use anyhow::Result;

pub async fn new_run(source_name: String) -> Result<()> {
    println!("Creating a new source...");

    let mut server_file = SERVER_FILE.write().await;

    // Check if source already exists
    if server_file.source_exists(&source_name)? {
        println!(" Source '{}' already exists!", source_name);
        return Ok(());
    }

    let src_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Add new source (automatically creates directory)
    server_file
        .add_source(src_id.clone(), source_name.clone(), timestamp.clone())
        .await?;

    println!(" Source '{}' created successfully!", source_name);

    Ok(())
}
