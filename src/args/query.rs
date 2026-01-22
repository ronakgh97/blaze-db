use crate::core::ClientConfig;
use crate::server::{QueryRequest, QueryResponse};
use anyhow::Result;
use colored::Colorize;

pub async fn query_run(database: String, src: String, query: String, top_k: usize) -> Result<()> {
    println!("\nSearch querying the database: {}\n", database.yellow());

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let request_body = QueryRequest {
        database,
        query,
        top_k,
        source: src,
    };

    let response = reqwest::Client::new()
        .post(config.url + "/v1/blaze/query")
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
