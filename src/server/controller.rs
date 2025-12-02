use crate::info;
use crate::prelude::log;
use crate::server::HealthCheckResponse;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::OnceLock;
use std::time::Instant;
#[allow(unused_imports)]
use uuid::Uuid;

static START_TIME: OnceLock<Instant> = OnceLock::new();

async fn create_router() -> Router {
    Router::new().route("/health", get(health_check))
}

pub async fn start_server() {
    START_TIME.get_or_init(|| Instant::now());

    let app = create_router().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8001")
        .await
        .unwrap();

    info!("Server is running on http://127.0.0.1:8001");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
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
