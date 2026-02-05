use crate::core::UserConfig;
use crate::server::{QueryRequest, QueryResponse};
use anyhow::Result;
use colored::Colorize;

pub async fn query_run(database: String, src: String, query: String, top_k: usize) -> Result<()> {
    println!("\nSearch querying the database: {}\n", database.yellow());

    let config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    let request_body = QueryRequest {
        database,
        query,
        top_k,
        source: src,
    };

    dotenv::dotenv().ok();
    let api_key = std::env::var("BLAZE_API_KEY").unwrap_or("local_dev_key".to_string());

    let response = reqwest::Client::new()
        .post(config.server.instance_url + "/v1/blazedb/query")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_result: QueryResponse = response.json().await?;

        for (i, item) in resp_result.results.iter().enumerate() {
            println!("\nItem {}:", i + 1);
            println!("Metadata: {}", item.chunk.to_string().green().dimmed());
            println!("Score: {:.4}", item.score.to_string().cyan());
        }
        println!(
            "Time taken (sec): {}",
            resp_result.search_time_sec.to_string().on_bright_yellow()
        );
    } else {
        println!("Failed to query database. Status: {}", response.status());
    }

    Ok(())
}
