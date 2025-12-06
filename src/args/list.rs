use crate::server::ListDatabasesResponse;
use anyhow::Result;

pub async fn list_run(source: Option<String>) -> Result<()> {
    println!("Listing all source dirs...");

    let response = reqwest::Client::new()
        .get("http://127.0.0.1:8001/databases")
        .send()
        .await?;

    if response.status().is_success() {
        let databases: Vec<ListDatabasesResponse> = response.json().await?;
        match source {
            Some(src) => {
                let get_source = databases.into_iter().find(|db| db.from_sources == src);
                println!("Source: {:?}", get_source);
            }
            None => {
                println!(" All Sources: {:?}", databases)
            }
        }
    } else {
        println!("Failed to list databases. Status: {}", response.status());
    }
    Ok(())
}
