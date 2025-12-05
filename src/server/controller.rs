use crate::prelude::log;
use crate::server::service::create_new_database;
use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse, HealthCheckResponse};
use crate::{error, info};
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
}

pub async fn start_server(port: u16, source: String) {
    START_TIME.get_or_init(|| Instant::now());
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

/// Health check handler
pub async fn health_check() -> impl IntoResponse {
    // Calculate uptime in seconds since server started
    let uptime_secs = START_TIME
        .get()
        .map(|start| start.elapsed().as_secs())
        .unwrap_or(0);

    let health = HealthCheckResponse {
        status: "OK".to_string(),
        service: "BlazeDB".to_string(),
        uptime_hrs: uptime_secs / 3600,
    };

    info!("health check ok, uptime: {}hr", uptime_secs / 3600);

    (StatusCode::OK, Json(health))
}

pub async fn create_database(Json(payload): Json<CreateDatabaseRequest>) -> impl IntoResponse {
    match create_new_database(payload.clone()).await {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(_) => {
            error!("Could not create database: {}", payload.name.clone());
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateDatabaseResponse {
                    id: "null".to_string(),
                    name: "null".to_string(),
                }),
            )
        }
    }
}
