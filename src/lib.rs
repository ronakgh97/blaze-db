mod args;
mod client_args;
pub mod core;
mod server;
mod server_args;
pub mod utils;

pub mod prelude {
    pub use crate::args::{
        bench_run, create_run, embed_run, init_run_client, init_run_server, list_run, print_ascii,
        query_run, register_run, serve_run,
    };
    pub use crate::client_args::{ClientArgs, ClientCommands};
    pub use crate::core::{
        Catalog, HNSW, Metrics, Node, NodeIndex, SERVER_FILE, Source, SyncReport, UserConfig,
        VectorBase, check_source_valid, cosine_similarity, dot_product, euclidean_similarity,
        get_source_path, list_sources, save_config,
    };
    pub use crate::server::{
        BackupInfoDto, CreateBackupRequest, CreateBackupResponse, CreateDatabaseRequest,
        CreateDatabaseResponse, CreateSourceRequest, CreateSourceResponse, DeleteBackupRequest,
        DeleteBackupResponse, EmbedData, EmbedRequest, EmbedResponse, InsertRequest,
        InsertResponse, ListBackupsRequest, ListBackupsResponse, ListResponse, QueryRequest,
        QueryResponse, RestoreBackupRequest, RestoreBackupResponse, VectorBaseObject,
        VectorDataDto, VectorQueryRequest, VectorQueryResponse, list_databases_from_disk,
        start_server,
    };
    pub use crate::server_args::{ServerArgs, ServerCommands};
    pub use crate::utils::{EmbeddingStore, Ingestor, Provider, VectorData, log};
}
