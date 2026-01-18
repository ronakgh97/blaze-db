use crate::core::ClientConfig;
use crate::server::ListDatabasesResponse;
use anyhow::Result;

pub async fn list_run() -> Result<()> {
    println!("Listing databases from active source...");

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let response = reqwest::Client::new()
        .get(config.url + "/v1/blaze/databases/list")
        .send()
        .await?;

    if response.status().is_success() {
        let databases: Vec<ListDatabasesResponse> = response.json().await?;

        if databases.is_empty() {
            println!("No databases found.");
        } else {
            for source_data in databases {
                println!("  Databases ({}):", source_data.databases.len());
                for db in source_data.databases {
                    println!("    • {}", db);
                }
            }
        }
    } else {
        println!("Failed to list databases. Status: {}", response.status());
    }
    Ok(())
}
