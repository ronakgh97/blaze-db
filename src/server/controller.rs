use crate::server::service::{create_new_database, embed_run, list_databases, query_search};
use crate::server::{
    CreateDatabaseRequest, CreateDatabaseResponse, EmbedRequest, EmbedResponse,
    HealthCheckResponse, ListDatabasesResponse, QueryRequest,
};
use crate::{error, info};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::OnceLock;
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();
static ACTIVE_SOURCE: OnceLock<String> = OnceLock::new();

async fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/create", post(create_database))
        .route("/embed", post(new_embeddings))
        .route("/databases", get(get_databases))
        .route("/query", post(search_query))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}

pub async fn start_server(port: u16, source: String) {
    START_TIME.get_or_init(Instant::now);
    ACTIVE_SOURCE.get_or_init(|| source.clone());

    info!("Server is running on http://127.0.0.1:{}", port);
    info!("Active source: {}", source);

    let app = create_router().await;
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Get the active source name for this server instance
pub fn get_active_source() -> Option<&'static str> {
    ACTIVE_SOURCE.get().map(|s| s.as_str())
}

/// Get the server uptime in hours
pub fn get_uptime_hrs() -> f32 {
    let uptime_secs = START_TIME
        .get()
        .map(|start| start.elapsed().as_secs_f32())
        .unwrap_or(0.0);

    (uptime_secs / 3600.0 * 10_000.0).round() / 10_000.0
}

/// Health check handler
pub async fn health_check() -> impl IntoResponse {
    let health = HealthCheckResponse {
        status: "OK".to_string(),
        service: "BlazeDB".to_string(),
        uptime_hrs: get_uptime_hrs(),
    };

    info!("health check ok, uptime: {}hr", get_uptime_hrs());

    (StatusCode::OK, Json(health))
}

pub async fn create_database(Json(payload): Json<CreateDatabaseRequest>) -> impl IntoResponse {
    info!(
        "[POST /create] Request to create database: '{}' with {} dimensions",
        payload.name, payload.dimensions
    );

    match create_new_database(payload.clone()).await {
        Ok(response) => {
            info!(
                "[POST /create] Database '{}' created successfully with ID: {}",
                response.name, response.id
            );
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            error!(
                "[POST /create] Failed to create database: {}",
                payload.name.clone()
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateDatabaseResponse {
                    id: "null".to_string(),
                    name: "null".to_string(),
                    source: "null".to_string(),
                    created_at: "null".to_string(),
                }),
            )
        }
    }
}

pub async fn new_embeddings(Json(payload): Json<EmbedRequest>) -> impl IntoResponse {
    let total_chunks: usize = payload.file_content.iter().map(|batch| batch.len()).sum();
    info!(
        "[POST /embed] Request to embed {} chunks into database '{}' with batch size {}",
        total_chunks, payload.database, payload.batch
    );

    match embed_run(payload.clone()).await {
        Ok(response) => {
            info!(
                "[POST /embed] Successfully embedded {} lines into database '{}'",
                response.total_lines, response.database
            );
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            error!(
                "[POST /embed] Failed to embed data into database: {}",
                payload.database.clone()
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(EmbedResponse {
                    database: "null".to_string(),
                    total_lines: 0,
                }),
            )
        }
    }
}

pub async fn get_databases() -> impl IntoResponse {
    info!("[GET /databases] Request to list all databases");

    match list_databases().await {
        Ok(response) => {
            let total_dbs: usize = response.iter().map(|r| r.databases.len()).sum();
            info!(
                "[GET /databases] Found {} databases across {} sources",
                total_dbs,
                response.len()
            );
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            error!("[GET /databases] Failed to list databases");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(vec![ListDatabasesResponse {
                    from_sources: "null".to_string(),
                    databases: vec![],
                }]),
            )
        }
    }
}

pub async fn search_query(Json(payload): Json<QueryRequest>) -> impl IntoResponse {
    info!(
        "[POST /query] Query request on database '{}': '{}' (top_k={})",
        payload.database, payload.query, payload.top_k
    );

    match query_search(payload.clone()).await {
        Ok(response) => {
            info!(
                "[POST /query] Query successful, returning {} results",
                response.len()
            );
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            error!(
                "[POST /query] Query failed on database: {}",
                payload.database.clone()
            );
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}
