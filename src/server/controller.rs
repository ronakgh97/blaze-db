use crate::server::{CreateDatabaseRequest, CreateDatabaseResponse};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

async fn create_router() -> Router {
    Router::new()
        .route("/", get(health_check))
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
    (
        StatusCode::OK,
        Json(json!({
            "status": "OK",
            "service": "blaze-db",
            "version": "1.0.0-beta"
        })),
    )
}

/// Handler that accepts JSON body via DTO
pub async fn create_database(Json(payload): Json<CreateDatabaseRequest>) -> impl IntoResponse {
    // Access the DTO fields
    let db_id = format!("db_{}", Uuid::new_v4());

    println!("Creating database: {}", payload.name);
    println!("Description: {:?}", payload.description);
    println!("Dimensions: {:?}", payload.dimensions);

    let response = CreateDatabaseResponse {
        id: db_id,
        name: payload.name,
    };

    (StatusCode::CREATED, Json(response))
}
