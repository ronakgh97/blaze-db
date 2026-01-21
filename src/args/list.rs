use crate::core::ClientConfig;
use crate::prelude::parse_database_name;
use crate::server::ListResponse;
use anyhow::Result;
use colored::Colorize;

pub async fn list_run() -> Result<()> {
    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let response = reqwest::Client::new()
        .get(config.url + "/v1/blaze/list")
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
                    // Parse db to get db name
                    let Some((db_name, _, _, _)) = parse_database_name(&db) else {
                        println!("    • {}", db.cyan());
                        continue;
                    };
                    println!("    • {}", db_name.cyan());
                }
            }
        }
    } else {
        println!("Failed to list databases. Status: {}", response.status());
    }
    Ok(())
}
