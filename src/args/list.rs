use crate::core::UserConfig;
use crate::server::ListResponse;
use anyhow::Result;
use colored::Colorize;

pub async fn list_run() -> Result<()> {
    let config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    dotenv::dotenv().ok();
    let api_key = std::env::var("BLAZE_API_KEY").unwrap_or("local_dev_key".to_string());

    let response = reqwest::Client::new()
        .get(config.server.instance_url + "/v1/blazedb/list")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    if response.status().is_success() {
        let list: Vec<ListResponse> = response.json().await?;

        if list.is_empty() {
            println!("No sources/databases found.");
        } else {
            for source_data in list {
                println!("  Sources ({})", source_data.from_sources.yellow());
                for db in source_data.databases {
                    println!("    • {}", db.name.cyan());
                }
            }
        }
    } else {
        println!("Failed to list databases. Status: {}", response.status());
    }
    Ok(())
}
