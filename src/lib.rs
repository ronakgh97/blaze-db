mod args;
mod client_args;
pub mod core;
mod server;
mod server_args;
pub mod utils;

pub mod prelude {
    pub use crate::args::{
        create_run, embed_run, init_run_client, init_run_server, list_run, new_run, print_ascii,
        query_run, serve_run,
    };
    pub use crate::client_args::{ClientArgs, ClientCommands};
    pub use crate::core::{
        ClientConfig, HNSW, Metrics, Node, NodeId, SERVER_FILE, SearchQuery, SearchResult,
        ServerFile, ServerStats, Source, SyncReport, VectorBase, check_source_valid,
        cosine_similarity, dot_product, euclidean_similarity, get_source_path, list_sources,
        save_config,
    };
    pub use crate::server::{
        CreateDatabaseRequest, CreateDatabaseResponse, CreateSourceRequest, CreateSourceResponse,
        EmbedRequest, EmbedResponse, InsertRequest, InsertResponse, ListResponse, QueryRequest,
        QueryResponse, VectorDataDto, list_databases_from_disk, parse_database_name, start_server,
    };
    pub use crate::server_args::{ServerArgs, ServerCommands};
    pub use crate::utils::{EmbeddingStore, Ingestor, Provider, VectorData, log};
}
