use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse, HealthCheckResponse};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

async fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/database", post(create_database))
}

pub async fn start_server() {
    let app = create_router().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8001")
        .await
        .unwrap();

    println!("Server starting on http://127.0.0.1:8001");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

/// Health check handler
pub async fn health_check() -> impl IntoResponse {
    let health = HealthCheckResponse {
        status: "OK".to_string(),
        service: "BlazeDB".to_string(),
        uptime: 3000,
    };

    (StatusCode::OK, Json(health))
}

/// Handler that accepts JSON body via DTO
pub async fn create_database(Json(payload): Json<CreateDatabaseRequest>) -> impl IntoResponse {
    // Access the DTO fields
    let db_id = format!("db_{}", Uuid::new_v4());

    let response = CreateDatabaseResponse {
        id: db_id,
        name: payload.name,
    };

    (StatusCode::CREATED, Json(response))
}
