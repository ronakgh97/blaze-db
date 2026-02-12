use crate::core::Metrics;
use crate::server::dto::{
    CreateSourceRequest, CreateSourceResponse, ListResponse, VectorQueryResponse,
};
use crate::server::service::{
    create_new_database, create_new_source, embed_run, insert_run, list_source, query_search,
    query_vector,
};
use crate::server::{
    CreateDatabaseRequest, CreateDatabaseResponse, EmbedRequest, EmbedResponse,
    HealthCheckResponse, InsertRequest, InsertResponse, QueryRequest, QueryResponse,
    VectorQueryRequest,
};
use crate::utils::{EmbeddingMetadata, EmbeddingStore, Provider};
use crate::{error, info, warn};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use lazy_static::lazy_static;
use lru::LruCache;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use std::fmt::Display;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, RwLock};

static START_TIME: OnceLock<chrono::DateTime<chrono::Local>> = OnceLock::new();
static PROVIDER: OnceLock<Provider> = OnceLock::new();

// pub static LOADED_INDEXES: OnceLock<Arc<Mutex<HashMap<String, EmbeddingStore>>>> = OnceLock::new();

lazy_static! {
    /// Per-database write locks to ensure only one write operation happens at a time per database
    /// Multiple databases can be written to concurrently, but only one write per database
    pub static ref DB_WRITE_LOCKS: Arc<Mutex<HashMap<String, Arc<RwLock<()>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    /// Per-database loading locks to prevent duplicate index loads during cache misses
    /// Ensures only one thread loads a specific database at a time, while allowing
    /// concurrent loading of different databases not same database!!
    pub static ref LOADING_LOCKS: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // TODO: Maybe use wrapper struct to keep most used metadata in memory too for faster access
    /// LRU Cache for loaded indexes to limit memory usage during queries
    /// Caches up to 12 databases in memory
    pub static ref INDEX_CACHE: Arc<RwLock<LruCache<String, (Arc<EmbeddingMetadata>, Arc<EmbeddingStore>)>>> =
        Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(12).unwrap() // Cache 12 databases
        )));
}

async fn create_router() -> Router {
    Router::new()
        .route("/v1/blazedb/health", get(health_check))
        .route("/v1/blazedb/databases/create", post(create_database))
        .route("/v1/blazedb/sources/create", post(create_src))
        .route("/v1/blazedb/list", get(list_sources))
        //.route("/v1/blaze/sources/del", del(delete_src)) TODO: Endpoint to delete source and all associated databases and indexes
        //.route("/v1/blaze/databases/del", det(delete_db)) TODO: Endpoint to delete database and all associated indexes
        //.route("/v1/blaze/vectors/del", det(delete_vectors)) TODO: Endpoint to delete specific vectors from a database
        .route("/v1/blazedb/insert", post(new_insert))
        .route("/v1/blazedb/query/vector", post(search_vector))
        .route("/v1/blazedb/embed", post(new_embeddings))
        .route("/v1/blazedb/query", post(search_query))
        .layer(DefaultBodyLimit::max(128 * 1024 * 1024))
}

// Start the server with the given port and multiple sources or single source
pub async fn start_server(
    port: u16,
    source: Vec<String>,
    provider: &Provider,
) -> anyhow::Result<()> {
    PROVIDER.set(provider.clone()).unwrap();

    // TODO: Add SourceManager to manage multiple sources dynamically and load/unload indexes as needed

    let addr = format!("0.0.0.0:{}", port);
    info!("Server is running on http://{}", addr);
    info!("Using Sources: {:?}", source);

    let app = create_router().await;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server_time = chrono::Local::now();

    // Initialize server start time
    START_TIME.get_or_init(|| server_time);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Get the server uptime in hours
pub fn get_uptime_hrs() -> f64 {
    let uptime_hrs = if let Some(start_time) = START_TIME.get() {
        let now = chrono::Local::now();
        let duration = now.signed_duration_since(*start_time);
        duration.num_hours() as f64
    } else {
        0.0
    };

    uptime_hrs
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

    // Checks here
    if !validate_empty_string(&payload.name)
        || !validate_empty_string(&payload.source)
        || payload.dimensions < 768
    {
        error!(
            "[POST /create] Invalid database creation request: name or source is empty or dimensions < 768"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateDatabaseResponse {
                id: "null".to_string(),
                name: "null".to_string(),
                metrics: Metrics::Cosine,
                dimensions: 0,
                source: "null".to_string(),
                created_at: "null".to_string(),
            }),
        );
    }

    match create_new_database(payload.clone()).await {
        Ok(response) => {
            info!(
                "[POST /create] Database '{}' created successfully with ID: {}",
                response.name, response.id
            );
            (StatusCode::CREATED, Json(response))
        }
        Err(e) => {
            if let Some(error_type) = e.downcast_ref::<ErrorTypes>() {
                match error_type {
                    ErrorTypes::SourceNotFound(msg) => {
                        error!(
                            "[POST /create] Source not found error during database creation: {}",
                            msg
                        );
                        return (
                            StatusCode::NO_CONTENT,
                            Json(CreateDatabaseResponse {
                                id: "null".to_string(),
                                name: "null".to_string(),
                                metrics: Metrics::Cosine,
                                dimensions: 0,
                                source: "null".to_string(),
                                created_at: "null".to_string(),
                            }),
                        );
                    }
                    ErrorTypes::DatabaseAlreadyExists(msg) => {
                        error!(
                            "[POST /create] Database already exists error during database creation: {}",
                            msg
                        );
                        return (
                            StatusCode::CONFLICT,
                            Json(CreateDatabaseResponse {
                                id: "null".to_string(),
                                name: "null".to_string(),
                                metrics: Metrics::Cosine,
                                dimensions: 0,
                                source: "null".to_string(),
                                created_at: "null".to_string(),
                            }),
                        );
                    }
                    ErrorTypes::InvalidField(msg) => {
                        error!(
                            "[POST /create] Invalid field error during database creation: {}",
                            msg
                        );
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(CreateDatabaseResponse {
                                id: "null".to_string(),
                                name: "null".to_string(),
                                metrics: Metrics::Cosine,
                                dimensions: 0,
                                source: "null".to_string(),
                                created_at: "null".to_string(),
                            }),
                        );
                    }

                    _ => {}
                }
            }

            error!(
                "[POST /create] Failed to create database: {} - Error: {:?}",
                payload.name.clone(),
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateDatabaseResponse {
                    id: "null".to_string(),
                    name: "null".to_string(),
                    metrics: Metrics::Cosine,
                    dimensions: 0,
                    source: "null".to_string(),
                    created_at: "null".to_string(),
                }),
            )
        }
    }
}

pub async fn create_src(Json(payload): Json<CreateSourceRequest>) -> impl IntoResponse {
    info!(
        "[POST /sources/create] Request to create source: '{}'",
        payload.source_name
    );

    // Check empty source name
    if !validate_empty_string(&payload.source_name) {
        error!("[POST /sources/create] Invalid source creation request: source name is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateSourceResponse {
                id: "null".to_string(),
                source: "null".to_string(),
                created_at: "null".to_string(),
            }),
        );
    }

    match create_new_source(payload.clone()).await {
        Ok(response) => {
            info!(
                "[POST /sources/create] Source '{}' created successfully",
                payload.source_name
            );
            (StatusCode::CREATED, Json(response))
        }
        Err(e) => {
            if let Some(error_type) = e.downcast_ref::<ErrorTypes>() {
                match error_type {
                    ErrorTypes::SourceAlreadyExists(msg) => {
                        warn!(
                            "[POST /sources/create] Source already exists error during source creation: {}",
                            msg
                        );
                        return (
                            StatusCode::CONFLICT,
                            Json(CreateSourceResponse {
                                id: "null".to_string(),
                                source: "null".to_string(),
                                created_at: "null".to_string(),
                            }),
                        );
                    }
                    _ => {}
                }
            }

            error!(
                "[POST /sources/create] Failed to create source: {} - Error: {:?}",
                payload.source_name.clone(),
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateSourceResponse {
                    id: "null".to_string(),
                    source: "null".to_string(),
                    created_at: "null".to_string(),
                }),
            )
        }
    }
}

pub async fn new_embeddings(Json(payload): Json<EmbedRequest>) -> impl IntoResponse {
    let total_chunks: usize = payload.batch_content.iter().map(|batch| batch.len()).sum();
    info!(
        "[POST /embed] Request to embed {} chunks into database '{}' with batch size {}",
        total_chunks, payload.database, payload.batch
    );

    // Check for empty's
    if payload.batch_content.is_empty() {
        error!("[POST /embed] Invalid embed request: batch_content is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(EmbedResponse {
                database: "null".to_string(),
                source: "null".to_string(),
                total_entries: 0,
            }),
        );
    }

    if !validate_empty_string(&payload.database) || !validate_empty_string(&payload.source) {
        error!("[POST /embed] Invalid embed request: database or source is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(EmbedResponse {
                database: "null".to_string(),
                source: "null".to_string(),
                total_entries: 0,
            }),
        );
    }

    // TODO: Maybe this is little overhead
    if payload
        .batch_content
        .par_iter()
        .any(|batch| batch.is_empty())
    {
        error!(
            "[POST /embed] Invalid embed request: one or more batches in batch_content is empty"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(EmbedResponse {
                database: "null".to_string(),
                source: "null".to_string(),
                total_entries: 0,
            }),
        );
    }

    match embed_run(payload.clone(), None, PROVIDER.wait()).await {
        Ok(response) => {
            info!(
                "[POST /embed] Successfully embedded {} lines into database '{}'",
                response.total_entries, response.database
            );
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            if let Some(error_type) = e.downcast_ref::<ErrorTypes>() {
                match error_type {
                    ErrorTypes::DatabaseNotFound(msg) => {
                        error!(
                            "[POST /embed] Database not found error during embed: {}",
                            msg
                        );
                        return (
                            StatusCode::NOT_FOUND,
                            Json(EmbedResponse {
                                database: "null".to_string(),
                                source: "null".to_string(),
                                total_entries: 0,
                            }),
                        );
                    }

                    ErrorTypes::SourceNotFound(msg) => {
                        error!("[POST /embed] Source not found error during embed: {}", msg);
                        return (
                            StatusCode::NOT_FOUND,
                            Json(EmbedResponse {
                                database: "null".to_string(),
                                source: "null".to_string(),
                                total_entries: 0,
                            }),
                        );
                    }

                    ErrorTypes::InvalidField(msg) => {
                        error!("[POST /embed] Invalid field error during embed: {}", msg);
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(EmbedResponse {
                                database: "null".to_string(),
                                source: "null".to_string(),
                                total_entries: 0,
                            }),
                        );
                    }
                    _ => {}
                }
            }

            error!(
                "[POST /embed] Failed to embed data into database: {} - Error: {:?}",
                payload.database.clone(),
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(EmbedResponse {
                    database: "null".to_string(),
                    source: "null".to_string(),
                    total_entries: 0,
                }),
            )
        }
    }
}

pub async fn new_insert(Json(payload): Json<InsertRequest>) -> impl IntoResponse {
    info!(
        "[POST /insert] Request to insert {} vectors into database '{}'",
        payload.nodes.len(),
        payload.database
    );

    // Check for empty's
    if payload.nodes.is_empty() {
        error!("[POST /insert] Invalid insert request: vectors array is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(InsertResponse {
                database: "null".to_string(),
                source: "null".to_string(),
                total_inserted: 0,
            }),
        );
    }

    if !validate_empty_string(&payload.database) || !validate_empty_string(&payload.source) {
        error!("[POST /insert] Invalid insert request: database or source is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(InsertResponse {
                database: "null".to_string(),
                source: "null".to_string(),
                total_inserted: 0,
            }),
        );
    }

    // TODO: Maybe this is little overhead
    if payload
        .nodes
        .par_iter()
        .any(|vector| vector.iter().any(|node| node.embedding.is_empty()))
    {
        error!(
            "[POST /insert] Invalid insert request: one or more vectors in nodes has empty embedding"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(InsertResponse {
                database: "null".to_string(),
                source: "null".to_string(),
                total_inserted: 0,
            }),
        );
    }

    match insert_run(&payload, None, PROVIDER.wait()).await {
        Ok(response) => {
            info!(
                "[POST /insert] Successfully inserted {} vectors into database '{}'",
                response.total_inserted, response.database
            );
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            if let Some(error_type) = e.downcast_ref::<ErrorTypes>() {
                match error_type {
                    ErrorTypes::DatabaseNotFound(msg) => {
                        error!(
                            "[POST /insert] Database not found error during insert: {}",
                            msg
                        );
                        return (
                            StatusCode::NOT_FOUND,
                            Json(InsertResponse {
                                database: "null".to_string(),
                                source: "null".to_string(),
                                total_inserted: 0,
                            }),
                        );
                    }

                    ErrorTypes::SourceNotFound(msg) => {
                        error!(
                            "[POST /insert] Source not found error during insert: {}",
                            msg
                        );
                        return (
                            StatusCode::NOT_FOUND,
                            Json(InsertResponse {
                                database: "null".to_string(),
                                source: "null".to_string(),
                                total_inserted: 0,
                            }),
                        );
                    }

                    ErrorTypes::InvalidField(msg) => {
                        error!("[POST /insert] Invalid field error during insert: {}", msg);
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(InsertResponse {
                                database: "null".to_string(),
                                source: "null".to_string(),
                                total_inserted: 0,
                            }),
                        );
                    }
                    _ => {}
                }
            }

            let db_name = &payload.database;
            error!(
                "[POST /insert] Failed to insert vectors into database: {} - Error: {:?}",
                db_name, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(InsertResponse {
                    database: "null".to_string(),
                    source: "null".to_string(),
                    total_inserted: 0,
                }),
            )
        }
    }
}

pub async fn list_sources() -> impl IntoResponse {
    info!("[GET /databases] Request to list all databases");

    match list_source().await {
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
                Json(vec![ListResponse {
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

    // Check for empty's
    if !validate_empty_string(&payload.query)
        || !validate_empty_string(&payload.database)
        || !validate_empty_string(&payload.source)
    {
        error!("[POST /query] Invalid query request: query, database, or source is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(QueryResponse {
                results: vec![],
                search_time_sec: 0.0,
                io_time_sec: 0.0,
            }),
        );
    }

    let db_name = payload.database.clone(); // Clone only the string for error logging
    match query_search(payload, PROVIDER.wait()).await {
        Ok(response) => {
            info!(
                "[POST /query] Query successful on database {}, returning {} results",
                db_name,
                response.results.len()
            );
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            if let Some(error_type) = e.downcast_ref::<ErrorTypes>() {
                match error_type {
                    ErrorTypes::DatabaseNotFound(msg) => {
                        error!(
                            "[POST /query] Database not found error during query: {}",
                            msg
                        );
                        return (
                            StatusCode::NO_CONTENT,
                            Json(QueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }

                    ErrorTypes::SourceNotFound(msg) => {
                        error!("[POST /query] Source not found error during query: {}", msg);
                        return (
                            StatusCode::NO_CONTENT,
                            Json(QueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }

                    ErrorTypes::IndexNotFound(msg) => {
                        error!("[POST /query] Index not found error during query: {}", msg);
                        return (
                            StatusCode::NO_CONTENT,
                            Json(QueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }
                    _ => {}
                }
            }
            error!(
                "[POST /query] Failed to query database: {} - Error: {:?}",
                db_name, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(QueryResponse {
                    results: vec![],
                    search_time_sec: 0.0,
                    io_time_sec: 0.0,
                }),
            )
        }
    }
}

pub async fn search_vector(Json(payload): Json<VectorQueryRequest>) -> impl IntoResponse {
    info!(
        "[POST /query/vector] Vector query request on database '{}' with {} dimensions (top_k={})",
        payload.database,
        payload.query_vector.len(),
        payload.top_k
    );

    // Check for empty's
    if payload.query_vector.is_empty()
        || !validate_empty_string(&payload.database)
        || !validate_empty_string(&payload.source)
    {
        error!(
            "[POST /query/vector] Invalid vector query request: query_vector is empty, or database/source is empty"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(VectorQueryResponse {
                results: vec![],
                search_time_sec: 0.0,
                io_time_sec: 0.0,
            }),
        );
    }

    let db_name = payload.database.clone();
    match query_vector(payload, PROVIDER.wait()).await {
        Ok(response) => {
            info!(
                "[POST /query/vector] Vector query successful on database '{}', returning {} results",
                db_name,
                response.results.len()
            );
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            if let Some(error_type) = e.downcast_ref::<ErrorTypes>() {
                match error_type {
                    ErrorTypes::DatabaseNotFound(msg) => {
                        error!(
                            "[POST /query/vector] Database not found error during vector query: {}",
                            msg
                        );
                        return (
                            StatusCode::NO_CONTENT,
                            Json(VectorQueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }

                    ErrorTypes::SourceNotFound(msg) => {
                        error!(
                            "[POST /query/vector] Source not found error during vector query: {}",
                            msg
                        );
                        return (
                            StatusCode::NO_CONTENT,
                            Json(VectorQueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }

                    ErrorTypes::IndexNotFound(msg) => {
                        error!(
                            "[POST /query/vector] Index not found error during vector query: {}",
                            msg
                        );
                        return (
                            StatusCode::NO_CONTENT,
                            Json(VectorQueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }

                    ErrorTypes::InvalidField(msg) => {
                        error!(
                            "[POST /query/vector] Invalid field error during vector query: {}",
                            msg
                        );
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(VectorQueryResponse {
                                results: vec![],
                                search_time_sec: 0.0,
                                io_time_sec: 0.0,
                            }),
                        );
                    }
                    _ => {}
                }
            }
            error!(
                "[POST /query/vector] Failed to execute vector query on database '{}' - Error: {:?}",
                db_name, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(VectorQueryResponse {
                    results: vec![],
                    search_time_sec: 0.0,
                    io_time_sec: 0.0,
                }),
            )
        }
    }
}

// pub async fn list_sources() -> impl IntoResponse {
//     (
//         StatusCode::NOT_IMPLEMENTED,
//         Json(ListSourcesResponse { sources: vec![] }),
//     )
// }
//
// pub async fn load_src(Json(_payload): Json<SourceLoadRequest>) -> impl IntoResponse {
//     (
//         StatusCode::NOT_IMPLEMENTED,
//         Json(SourceLoadResponse {
//             source: "".to_string(),
//             database: "".to_string(),
//             total_index: 0,
//         }),
//     )
// }
//
// pub async fn unload_src(Json(_payload): Json<SourceUnloadRequest>) -> impl IntoResponse {
//     (
//         StatusCode::NOT_IMPLEMENTED,
//         Json(SourceUnloadResponse {
//             source: "".to_string(),
//             database: "".to_string(),
//             total_unloaded: 0,
//         }),
//     )
// }

#[derive(Debug)]
pub enum ErrorTypes {
    DatabaseNotFound(String),
    SourceNotFound(String),
    IndexNotFound(String),
    DatabaseAlreadyExists(String),
    SourceAlreadyExists(String),
    InvalidField(String),
}

impl Display for ErrorTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorTypes::DatabaseNotFound(msg) => write!(f, "Database not found: {}", msg),
            ErrorTypes::SourceNotFound(msg) => write!(f, "Source not found: {}", msg),
            ErrorTypes::IndexNotFound(msg) => write!(f, "Index not found: {}", msg),
            ErrorTypes::DatabaseAlreadyExists(msg) => {
                write!(f, "Database already exists: {}", msg)
            }
            ErrorTypes::SourceAlreadyExists(msg) => write!(f, "Source already exists: {}", msg),
            ErrorTypes::InvalidField(msg) => write!(f, "Invalid field: {}", msg),
        }
    }
}

impl std::error::Error for ErrorTypes {}

fn validate_empty_string(field: &str) -> bool {
    if field.trim().is_empty() {
        return false;
    }
    true
}
