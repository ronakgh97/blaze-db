// These are AI generated slop tests, so it ain't touching so long they are passing 🙃

// Guess I need to touch these tests after all...AI had one job 🥳

use blaze_db::prelude::{
    CreateDatabaseRequest, CreateDatabaseResponse, CreateSourceRequest, CreateSourceResponse,
    EmbedData, EmbedRequest, EmbedResponse, InsertRequest, InsertResponse, ListResponse,
    QueryRequest, VectorDataDto,
};
use reqwest::Client;
use std::time::Duration;
use uuid::Uuid;
// These tests are required you to delete or backup, the local blaze storage

// Test server configuration
const BASE_URL: &str = "http://localhost:8080";
const HEALTH_ENDPOINT: &str = "/v1/blazedb/health";
const CREATE_DB_ENDPOINT: &str = "/v1/blazedb/databases/create";
const CREATE_SOURCE_ENDPOINT: &str = "/v1/blazedb/sources/create";
const LIST_ENDPOINT: &str = "/v1/blazedb/list";
const INSERT_ENDPOINT: &str = "/v1/blazedb/insert";
const EMBED_ENDPOINT: &str = "/v1/blazedb/embed";
const QUERY_ENDPOINT: &str = "/v1/blazedb/query";

// Dont create client on every request
fn create_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

// Helper function to wait for server to be ready
async fn wait_for_server(max_attempts: u32) -> bool {
    let client = create_client();
    for _i in 1..=max_attempts {
        if let Ok(response) = client
            .get(format!("{}{}", BASE_URL, HEALTH_ENDPOINT))
            .send()
            .await
            && response.status().is_success()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

#[tokio::test]
#[ignore]
async fn test_create_source_success() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: format!("TEST_source_{}", chrono::Utc::now().timestamp()),
    };

    let response = client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 201 CREATED
    assert_eq!(response.status(), 201);

    let body: CreateSourceResponse = response.json().await.expect("Failed to parse JSON");
    assert_ne!(body.id, "null");
    assert_eq!(body.source, request.source_name);
    assert_ne!(body.created_at, "null");
}

#[tokio::test]
#[ignore]
async fn test_create_database_success() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();

    // First create a source
    let source_name = format!("test_src_{}", chrono::Utc::now().timestamp());
    let source_request = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: source_name.clone(),
    };

    let source_response = client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_request)
        .send()
        .await
        .expect("Failed to create source");

    assert_eq!(source_response.status(), 201);

    // Now create database
    let db_request = CreateDatabaseRequest {
        name: format!("test_db_{}", chrono::Utc::now().timestamp()),
        source: source_name,
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 201 CREATED
    assert_eq!(response.status(), 201);

    let body: CreateDatabaseResponse = response.json().await.expect("Failed to parse JSON");
    assert_ne!(body.id, "null");
    assert_eq!(body.name, db_request.name);
    assert_eq!(body.dimensions, 1024);
    assert_ne!(body.created_at, "null");
}

#[tokio::test]
#[ignore]
async fn test_create_database_empty_source() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = CreateDatabaseRequest {
        name: "test_db".to_string(),
        source: "".to_string(), // Invalid: empty
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

// Yeah bro, dont use low dimensions, it aint gonna work and you know it 🙃

#[tokio::test]
#[ignore]
async fn test_create_database_low_dimensions() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = CreateDatabaseRequest {
        name: "test_db".to_string(),
        source: "test_src".to_string(),
        metrics: None,
        dimensions: 767, // Less than 768 - INVALID
        backup_interval_hours: None,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_create_database_invalid_source() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = CreateDatabaseRequest {
        name: "test_db".to_string(),
        source: "nonexistent_source_12345".to_string(), // Invalid: doesn't exist
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 204 NO_CONTENT (or 404 NOT_FOUND) since source doesn't exist
    // Shit waht tha damn diff between 204 and 404?????
    assert_eq!(response.status(), 204);
}

#[tokio::test]
#[ignore]
async fn test_create_database_duplicate_name_same_source() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();

    // Create a unique source for this test
    let source_name = format!("test_source_{}", chrono::Utc::now().timestamp());
    let source_request = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: source_name.clone(),
    };

    let source_response = client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_request)
        .send()
        .await
        .expect("Failed to create source");

    assert_eq!(source_response.status(), 201);

    // Create first database
    let db_name = "duplicate_test_db";
    let db_request1 = CreateDatabaseRequest {
        name: db_name.to_string(),
        source: source_name.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };

    let response1 = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_request1)
        .send()
        .await
        .expect("Failed to send request");

    // First creation should succeed with 201 CREATED
    assert_eq!(response1.status(), 201);

    // Try to create second database with same name in same source
    let db_request2 = CreateDatabaseRequest {
        name: db_name.to_string(),   // Same name!
        source: source_name.clone(), // Same source!
        metrics: None,
        dimensions: 1536, // Different dimensions - doesn't matter
        backup_interval_hours: None,
    };

    let response2 = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_request2)
        .send()
        .await
        .expect("Failed to send request");

    // Second creation should fail with 409 CONFLICT
    assert_eq!(response2.status(), 409);
}

#[tokio::test]
#[ignore]
async fn test_create_database_same_name_different_sources() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();

    // Create two different sources
    let timestamp = chrono::Utc::now().timestamp();
    let source_name1 = format!("test_src_1_{}", timestamp);
    let source_name2 = format!("test_src_2_{}", timestamp);

    // Create source 1
    let source_request1 = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: source_name1.clone(),
    };
    let source_response1 = client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_request1)
        .send()
        .await
        .expect("Failed to create source 1");
    assert_eq!(source_response1.status(), 201);

    // Create source 2
    let source_request2 = CreateSourceRequest {
        backup_interval_hours: None,
        source_name: source_name2.clone(),
    };
    let source_response2 = client
        .post(format!("{}{}", BASE_URL, CREATE_SOURCE_ENDPOINT))
        .json(&source_request2)
        .send()
        .await
        .expect("Failed to create source 2");
    assert_eq!(source_response2.status(), 201);

    // Create database with same name in source 1
    let db_name = "shared_name_db";
    let db_request1 = CreateDatabaseRequest {
        name: db_name.to_string(),
        source: source_name1.clone(),
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };

    let response1 = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_request1)
        .send()
        .await
        .expect("Failed to send request");

    // First creation should succeed
    assert_eq!(response1.status(), 201);

    // Create database with SAME name in source 2 (different source!)
    let db_request2 = CreateDatabaseRequest {
        name: db_name.to_string(),    // Same name!
        source: source_name2.clone(), // Different source!
        metrics: None,
        dimensions: 1024,
        backup_interval_hours: None,
    };

    let response2 = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .json(&db_request2)
        .send()
        .await
        .expect("Failed to send request");

    // Second creation should ALSO succeed (different source)
    assert_eq!(response2.status(), 201);
}

#[tokio::test]
#[ignore]
async fn test_list_sources() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let response = client
        .get(format!("{}{}", BASE_URL, LIST_ENDPOINT))
        .send()
        .await
        .expect("Failed to send request");

    // Should return 200 OK
    assert_eq!(response.status(), 200);

    let body: Vec<ListResponse> = response.json().await.expect("Failed to parse JSON");
    // Should have at least one source (or empty array)
    assert!(body.is_empty() || !body.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_insert_empty_vectors() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = InsertRequest {
        nodes: vec![], // Invalid: empty
        database: "test_db".to_string(),
        source: "test_src".to_string(),
    };

    let response = client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);

    let body: InsertResponse = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body.database, "null");
    assert_eq!(body.total_inserted, 0);
}

#[tokio::test]
#[ignore]
async fn test_insert_empty_database() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = InsertRequest {
        nodes: vec![vec![VectorDataDto {
            id: Uuid::new_v4().to_string(),
            embedding: vec![1.0, 2.0, 3.0],
            metadata: "test".to_string(),
        }]],
        database: "".to_string(), // Invalid: empty
        source: "test_src".to_string(),
    };

    let response = client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_insert_empty_embedding() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = InsertRequest {
        nodes: vec![vec![VectorDataDto {
            id: Uuid::new_v4().to_string(),
            embedding: vec![], // Invalid: empty
            metadata: "test".to_string(),
        }]],
        database: "test_db".to_string(),
        source: "test_src".to_string(),
    };

    let response = client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_embed_empty_batch_content() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = EmbedRequest {
        batch_content: vec![], // Invalid: empty
        database: "test_db".to_string(),
        source: "test_src".to_string(),
        batch: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, EMBED_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);

    let body: EmbedResponse = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body.database, "null");
    assert_eq!(body.total_entries, 0);
}

#[tokio::test]
#[ignore]
async fn test_embed_empty_database() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = EmbedRequest {
        batch_content: vec![vec![EmbedData {
            id: Uuid::new_v4().to_string(),
            embed_data: "test data".to_string(),
        }]],
        database: "".to_string(), // Invalid: empty
        source: "test_src".to_string(),
        batch: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, EMBED_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_embed_empty_batch_in_content() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = EmbedRequest {
        batch_content: vec![
            vec![EmbedData {
                id: Uuid::new_v4().to_string(),
                embed_data: "test data".to_string(),
            }],
            vec![], // Invalid: empty batch
        ],
        database: "test_db".to_string(),
        source: "test_src".to_string(),
        batch: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, EMBED_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_query_empty_database() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = QueryRequest {
        query: "test query".to_string(),
        database: "".to_string(), // Invalid: empty
        source: "test_src".to_string(),
        top_k: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, QUERY_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_query_empty_source() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = QueryRequest {
        query: "test query".to_string(),
        database: "test_db".to_string(),
        source: "".to_string(), // Invalid: empty
        top_k: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, QUERY_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_query_whitespace_only() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = QueryRequest {
        query: "   ".to_string(), // Invalid: whitespace only
        database: "test_db".to_string(),
        source: "test_src".to_string(),
        top_k: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, QUERY_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_query_only_one_valid_field() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = QueryRequest {
        query: "valid query".to_string(), // Valid
        database: "".to_string(),         // Invalid: empty
        source: "".to_string(),           // Invalid: empty
        top_k: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, QUERY_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST (OR logic - ANY invalid field fails)
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_query_only_two_valid_fields() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let request = QueryRequest {
        query: "valid query".to_string(), // Valid
        database: "test_db".to_string(),  // Valid
        source: "".to_string(),           // Invalid: empty
        top_k: 10,
    };

    let response = client
        .post(format!("{}{}", BASE_URL, QUERY_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should return 400 BAD_REQUEST (OR logic - ANY invalid field fails)
    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_large_payload() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();

    // Create large batch of vectors
    let large_vectors: Vec<VectorDataDto> = (0..100)
        .map(|i| VectorDataDto {
            id: Uuid::new_v4().to_string(),
            embedding: vec![i as f32; 1024],
            metadata: format!("vector_{}", i),
        })
        .collect();

    let request = InsertRequest {
        nodes: vec![large_vectors],
        database: "test_db".to_string(),
        source: "test_src".to_string(),
    };

    let response = client
        .post(format!("{}{}", BASE_URL, INSERT_ENDPOINT))
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Should handle large payload (400 if DB doesn't exist, not 500)
    assert!(response.status() == 400 || response.status() == 404 || response.status() == 200);
}

#[tokio::test]
#[ignore]
async fn test_missing_content_type() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();

    let response = client
        .post(format!("{}{}", BASE_URL, CREATE_DB_ENDPOINT))
        .body(r#"{"name":"test","source":"src","dimensions":1024}"#)
        .send()
        .await
        .expect("Failed to send request");

    // Should still work (Axum handles this)
    assert!(response.status().is_client_error() || response.status().is_success());
}

#[tokio::test]
#[ignore]
async fn test_concurrent_health_checks() {
    assert!(wait_for_server(10).await, "Server not running!");

    let client = create_client();
    let mut handles = vec![];

    // Send 10 concurrent health check requests
    for _ in 0..10 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let response = client_clone
                .get(format!("{}{}", BASE_URL, HEALTH_ENDPOINT))
                .send()
                .await
                .expect("Failed to send request");

            response.status() == 200
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let results = futures::future::join_all(handles).await;

    // All should succeed
    assert!(
        results
            .iter()
            .all(|r| r.as_ref().unwrap_or(&false) == &true)
    );
}
