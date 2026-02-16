use crate::core::Metrics;
use crate::server::dto::{
    BackupInfoDto, CreateBackupRequest, CreateBackupResponse, CreateSourceRequest,
    CreateSourceResponse, DeleteBackupRequest, DeleteBackupResponse, ListBackupsRequest,
    ListBackupsResponse, ListResponse, RestoreBackupRequest, RestoreBackupResponse,
    VectorQueryResponse,
};
use crate::server::service::{
    BackupConfig, BackupService, create_new_database, create_new_source, embed_run, insert_run,
    list_source, query_search, query_vector,
};
use crate::server::{
    CreateDatabaseRequest, CreateDatabaseResponse, EmbedRequest, EmbedResponse,
    HealthCheckResponse, InsertRequest, InsertResponse, QueryRequest, QueryResponse,
    VectorQueryRequest,
};
use crate::utils::{BackupInfo, EmbeddingMetadata, EmbeddingStore, Provider};
use crate::{error, info, warn};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use lazy_static::lazy_static;
use lru::LruCache;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::fmt::Display;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, RwLock};

static START_TIME: OnceLock<chrono::DateTime<chrono::Local>> = OnceLock::new();
static PROVIDER: OnceLock<Provider> = OnceLock::new();
static BACKUP_SERVICE: OnceLock<Arc<RwLock<BackupService>>> = OnceLock::new();

// pub static LOADED_INDEXES: OnceLock<Arc<Mutex<HashMap<String, EmbeddingStore>>>> = OnceLock::new();

lazy_static! {
    /// Per-database write locks with LRU eviction (cap: 1000 locks)
    /// Automatically evicts least-recently-used locks when capacity is reached
    /// Key format: "source:database" for consistency
    pub static ref DB_WRITE_LOCKS: Arc<RwLock<LruCache<String, Arc<RwLock<()>>>>> =
        Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(4096).unwrap() // Max 4096 concurrent database locks
        )));

    /// Per-database loading locks with LRU eviction (cap: 1000 locks)
    /// Prevents duplicate index loads during cache misses
    pub static ref LOADING_LOCKS: Arc<RwLock<LruCache<String, Arc<Mutex<()>>>>> =
        Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(4096).unwrap() // Max 4096 concurrent loading locks
        )));

    // TODO: Maybe use wrapper struct to keep most used metadata in memory too for faster access
    /// LRU Cache for loaded indexes to limit memory usage during queries
    /// Caches up to 12 databases in memory
    pub static ref INDEX_CACHE: Arc<RwLock<LruCache<String, (Arc<EmbeddingMetadata>, Arc<EmbeddingStore>)>>> =
        Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(128).unwrap() // Cache upto 128 index
        )));
}

async fn create_router() -> Router {
    Router::new()
        .route("/v1/blazedb/health", get(health_check))
        .route("/v1/blazedb/databases/create", post(create_database))
        .route("/v1/blazedb/sources/create", post(create_src))
        .route("/v1/blazedb/list", get(list_sources))
        .route("/v1/blazedb/backup/create", post(create_backup))
        .route("/v1/blazedb/backup/list", post(list_backups))
        .route("/v1/blazedb/backup/restore", post(restore_backup))
        .route("/v1/blazedb/backup/delete", post(delete_backup))
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
    run_backup_scheduler: bool,
    provider: &Provider,
) -> anyhow::Result<()> {
    PROVIDER.set(provider.clone()).unwrap();

    if run_backup_scheduler {
        // Initialize backup service
        let backup_config = BackupConfig::default();
        let mut backup_service = BackupService::new(backup_config);
        backup_service.start_scheduler().await;
        BACKUP_SERVICE
            .set(Arc::new(RwLock::new(backup_service)))
            .map_err(|_| anyhow::anyhow!("Failed to initialize backup service"))?;
    }

    // TODO: Add SourceManager to manage multiple sources dynamically and load/unload indexes as needed

    let addr = format!("0.0.0.0:{}", port);
    info!("Server is running on http://{}", addr);
    info!("Using {} Sources", source.len());
    info!("Backup scheduler enabled: {}", run_backup_scheduler);

    info!("Index cache capacity: {}", 128);

    let app = create_router().await;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server_time = chrono::Local::now();

    // Initialize server start time
    START_TIME.get_or_init(|| server_time);

    let shutdown_signal = setup_shutdown_signal();

    // Create server and wrap with graceful shutdown
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal);

    info!("Server started");
    server.await?;

    // Graceful shutdown cleanup
    info!("Server shutting down gracefully...");
    cleanup_on_shutdown().await;

    Ok(())
}

/// Setup signal handlers for graceful shutdown (Unix: SIGTERM/SIGINT, Windows: Ctrl+C)
async fn setup_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(_) => {
                return;
            }
        };

        tokio::select! {
            _ = sigterm.recv() => {
            }
            _ = sigint.recv() => {
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Windows or other non-Unix systems
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(_) => {}
        }
    }
}

/// Cleanup resources on graceful shutdown
async fn cleanup_on_shutdown() {
    // Stop backup scheduler if it was started
    if let Some(backup_service_lock) = BACKUP_SERVICE.get() {
        info!("Stopping schedulers..");
        let mut backup_service = backup_service_lock.write().await;
        backup_service.stop_scheduler().await;
        info!("Backup scheduler stopped");
    }

    // Log final lock counts (LRU auto-cleans, no manual cleanup needed)
    {
        let locks = DB_WRITE_LOCKS.read().await;
        info!("Server shutdown: {} DB_WRITE_LOCKS active", locks.len());
    }

    {
        let locks = LOADING_LOCKS.read().await;
        info!("Server shutdown: {} LOADING_LOCKS active", locks.len());
    }

    info!("Server shutdown complete");
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

pub async fn create_backup(Json(payload): Json<CreateBackupRequest>) -> impl IntoResponse {
    info!(
        "[POST /backup/create] Backup request for {}:{}",
        payload.source, payload.database
    );

    // Validation
    if !validate_empty_string(&payload.source) || !validate_empty_string(&payload.database) {
        error!("[POST /backup/create] Invalid request: source or database is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateBackupResponse {
                success: false,
                backup_info: None,
                message: "Source and database names are required".to_string(),
            }),
        );
    }

    // Get backup service - check if it was initialized
    let backup_service = match BACKUP_SERVICE.get() {
        Some(service) => service,
        None => {
            error!(
                "[POST /backup/create] Backup service not initialized - start server with --backup flag"
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(CreateBackupResponse {
                    success: false,
                    backup_info: None,
                    message: "Backup service not enabled".to_string(),
                }),
            );
        }
    };

    let service = backup_service.read().await;

    match service
        .trigger_backup(&payload.source, &payload.database)
        .await
    {
        Ok(backup_info) => {
            info!(
                "[POST /backup/create] Backup created successfully: {} ({} MB)",
                backup_info.file_name, backup_info.size_mb
            );
            (
                StatusCode::CREATED,
                Json(CreateBackupResponse {
                    success: true,
                    backup_info: Some(convert_backup_info(
                        backup_info,
                        &payload.source,
                        &payload.database,
                    )),
                    message: "Backup created successfully".to_string(),
                }),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            error!(
                "[POST /backup/create] Backup failed for {}:{} - {}",
                payload.source, payload.database, error_msg
            );
            (
                StatusCode::CONFLICT,
                Json(CreateBackupResponse {
                    success: false,
                    backup_info: None,
                    message: error_msg,
                }),
            )
        }
    }
}

pub async fn list_backups(Json(payload): Json<ListBackupsRequest>) -> impl IntoResponse {
    info!(
        "[POST /backup/list] List backups request for {}:{}",
        payload.source, payload.database
    );

    if !validate_empty_string(&payload.source) || !validate_empty_string(&payload.database) {
        error!("[POST /backup/list] Invalid request: source or database is empty");
        return (
            StatusCode::BAD_REQUEST,
            Json(ListBackupsResponse { backups: vec![] }),
        );
    }

    // Get backup service - check if it was initialized
    let backup_service = match BACKUP_SERVICE.get() {
        Some(service) => service,
        None => {
            error!(
                "[POST /backup/list] Backup service not initialized - start server with --backup flag"
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ListBackupsResponse { backups: vec![] }),
            );
        }
    };

    let service = backup_service.read().await;

    match service
        .list_backups(&payload.source, &payload.database)
        .await
    {
        Ok(backups) => {
            let backup_dtos: Vec<BackupInfoDto> = backups
                .into_iter()
                .map(|b| convert_backup_info(b, &payload.source, &payload.database))
                .collect();

            info!(
                "[POST /backup/list] Found {} backups for {}:{}",
                backup_dtos.len(),
                payload.source,
                payload.database
            );
            (
                StatusCode::OK,
                Json(ListBackupsResponse {
                    backups: backup_dtos,
                }),
            )
        }
        Err(e) => {
            error!(
                "[POST /backup/list] Failed to list backups for {}:{} - {}",
                payload.source, payload.database, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ListBackupsResponse { backups: vec![] }),
            )
        }
    }
}

pub async fn restore_backup(Json(payload): Json<RestoreBackupRequest>) -> impl IntoResponse {
    info!(
        "[POST /backup/restore] Restore request for {}:{} from {}",
        payload.source, payload.database, payload.backup_filename
    );

    if !validate_empty_string(&payload.source)
        || !validate_empty_string(&payload.database)
        || !validate_empty_string(&payload.backup_filename)
    {
        error!("[POST /backup/restore] Invalid request: missing required fields");
        return (
            StatusCode::BAD_REQUEST,
            Json(RestoreBackupResponse {
                success: false,
                message: "Source, database, and backup filename are required".to_string(),
            }),
        );
    }

    // Get backup service - check if it was initialized
    let backup_service = match BACKUP_SERVICE.get() {
        Some(service) => service,
        None => {
            error!(
                "[POST /backup/restore] Backup service not initialized - start server with --backup flag"
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(RestoreBackupResponse {
                    success: false,
                    message: "Backup service not enabled".to_string(),
                }),
            );
        }
    };

    let service = backup_service.read().await;

    match service
        .restore_backup(&payload.source, &payload.database, &payload.backup_filename)
        .await
    {
        Ok(()) => {
            info!(
                "[POST /backup/restore] Successfully restored {}:{} from {}",
                payload.source, payload.database, payload.backup_filename
            );
            (
                StatusCode::OK,
                Json(RestoreBackupResponse {
                    success: true,
                    message: "Database restored successfully".to_string(),
                }),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            error!(
                "[POST /backup/restore] Restore failed for {}:{} - {}",
                payload.source, payload.database, error_msg
            );
            (
                StatusCode::CONFLICT,
                Json(RestoreBackupResponse {
                    success: false,
                    message: error_msg,
                }),
            )
        }
    }
}

pub async fn delete_backup(Json(payload): Json<DeleteBackupRequest>) -> impl IntoResponse {
    info!(
        "[POST /backup/delete] Delete backup request: {} for {}:{}",
        payload.backup_filename, payload.source, payload.database
    );

    if !validate_empty_string(&payload.source)
        || !validate_empty_string(&payload.database)
        || !validate_empty_string(&payload.backup_filename)
    {
        error!("[POST /backup/delete] Invalid request: missing required fields");
        return (
            StatusCode::BAD_REQUEST,
            Json(DeleteBackupResponse {
                success: false,
                message: "Source, database, and backup filename are required".to_string(),
            }),
        );
    }

    // Get backup service - check if it was initialized
    let backup_service = match BACKUP_SERVICE.get() {
        Some(service) => service,
        None => {
            error!(
                "[POST /backup/delete] Backup service not initialized - start server with --backup flag"
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(DeleteBackupResponse {
                    success: false,
                    message: "Backup service not enabled".to_string(),
                }),
            );
        }
    };

    let service = backup_service.read().await;

    match service
        .delete_backup(&payload.source, &payload.database, &payload.backup_filename)
        .await
    {
        Ok(()) => {
            info!(
                "[POST /backup/delete] Successfully deleted {} for {}:{}",
                payload.backup_filename, payload.source, payload.database
            );
            (
                StatusCode::OK,
                Json(DeleteBackupResponse {
                    success: true,
                    message: "Backup deleted successfully".to_string(),
                }),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            error!(
                "[POST /backup/delete] Delete failed for {}:{} - {}",
                payload.source, payload.database, error_msg
            );
            (
                StatusCode::NOT_FOUND,
                Json(DeleteBackupResponse {
                    success: false,
                    message: error_msg,
                }),
            )
        }
    }
}

fn convert_backup_info(info: BackupInfo, source: &str, database: &str) -> BackupInfoDto {
    BackupInfoDto {
        filename: info.file_name,
        timestamp: info.timestamp,
        size_mb: info.size_mb,
        source: source.to_string(),
        database: database.to_string(),
    }
}
