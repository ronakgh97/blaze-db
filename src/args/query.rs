use crate::core::ClientConfig;
use crate::server::{QueryRequest, QueryResponse};
use anyhow::Result;
use colored::Colorize;

pub async fn query_run(database: String, query: String, top_k: usize) -> Result<()> {
    println!("Search querying the database...: {}", database);

    let config = ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await?;

    let request_body = QueryRequest {
        database,
        query,
        top_k,
    };

    let response = reqwest::Client::new()
        .post(config.url + "/v1/blaze/query")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_result: QueryResponse = response.json().await?;
        println!("Query Results:");

        for (i, item) in resp_result.results.iter().enumerate() {
            println!("\nResult {}:", i + 1);
            println!("Metadata: {}", item.chunk.to_string().green().dimmed());
            println!("Score: {:.4}", item.score.to_string().cyan());
        }
        println!(
            "Time taken (sec): {}",
            resp_result.time_ms.to_string().on_bright_yellow()
        );
    } else {
        println!("Failed to query database. Status: {}", response.status());
    }

    Ok(())
}
