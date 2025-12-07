use crate::server::{QueryRequest, QueryResponse};
use anyhow::Result;

pub async fn query_run(database: String, query: String, top_k: usize) -> Result<()> {
    println!("Search querying the database...: {}", database);

    let request_body = QueryRequest {
        database,
        query,
        top_k,
    };

    let response = reqwest::Client::new()
        .post("http://127.0.0.1:8001/query")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_result: Vec<QueryResponse> = response.json().await?;
        println!("Query Results:");

        for (i, item) in resp_result.iter().enumerate() {
            println!("\nResult {}:", i + 1);
            println!("Chunk: {}", item.chunk);
            println!("Score: {:.4}", item.score);
        }
    } else {
        println!("Failed to query database. Status: {}", response.status());
    }

    Ok(())
}
