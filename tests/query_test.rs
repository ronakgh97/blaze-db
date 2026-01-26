use blaze_db::prelude::{QueryRequest, QueryResponse};

// This test requires for server to be running, and amazon product index created.
#[ignore]
#[tokio::test]
async fn test_cache_and_bench() -> anyhow::Result<()> {
    // Send a query request to the server
    let client = reqwest::Client::new();

    let query_request = QueryRequest {
        query: "Gaming RTX 4060 Laptop with 165Hz Display ".to_string(),
        database: "test_db".to_string(),
        source: "default_src".to_string(),
        top_k: 5,
    };

    let time_taken_no_cache = std::time::Instant::now();
    let resp = client
        .post("http://localhost:8080/v1/blaze/query")
        .json(&query_request)
        .send()
        .await?;
    let client_elapsed_no_cache = time_taken_no_cache.elapsed().as_secs_f64();

    // Crash early if request failed
    assert!(
        resp.status().is_success(),
        "Request failed with status: {}",
        resp.status()
    );

    let query_response: QueryResponse = resp.json().await?;

    // Get search time from response
    let server_reported_time = query_response.search_time_sec;

    // Get IO time from response
    let server_reported_io_time = query_response.io_time_sec;

    let total_time_no_cache_server = server_reported_time + server_reported_io_time;

    let all_total_client_server_no_cache = client_elapsed_no_cache + total_time_no_cache_server;

    println!(
        "Total time without cache: {}s (Client: {}s, Server: {}s)",
        all_total_client_server_no_cache, client_elapsed_no_cache, total_time_no_cache_server
    );

    // Maybe wait a bit to ensure cache is ready
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Send a query request to the server again to test cache
    let time_taken_with_cache = std::time::Instant::now();
    let resp_cached = client
        .post("http://localhost:8080/v1/blaze/query")
        .json(&query_request)
        .send()
        .await?;
    let client_elapsed_with_cache = time_taken_with_cache.elapsed().as_secs_f64();

    // Crash early if request failed
    assert!(
        resp_cached.status().is_success(),
        "Request failed with status: {}",
        resp_cached.status()
    );

    let query_response_cached: QueryResponse = resp_cached.json().await?;

    // Get search time from response
    let server_reported_time_cached = query_response_cached.search_time_sec;

    // Get IO time from response
    let server_reported_io_time_cached = query_response_cached.io_time_sec;

    let total_time_with_cache_server = server_reported_time_cached + server_reported_io_time_cached;

    let all_total_client_server_with_cache =
        client_elapsed_with_cache + total_time_with_cache_server;

    println!(
        "Total time with cache: {}s (Client: {}s, Server: {}s)",
        all_total_client_server_with_cache, client_elapsed_with_cache, total_time_with_cache_server
    );

    // Ensure that the cached query is faster than the non-cached one
    assert!(
        total_time_with_cache_server < total_time_no_cache_server,
        "Cached query was not faster than non-cached query"
    );

    let metrics = total_time_no_cache_server / total_time_with_cache_server;
    println!("Improvement factor (Server side): {:.2}x", metrics);

    Ok(())
}
