mod args;
mod client_args;
mod core;
mod server;
mod server_args;
pub mod utils;

pub mod prelude {
    pub use crate::args::{
        create_run, embed_run, init_run, list_run, new_run, print_ascii, serve_run,
    };
    pub use crate::client_args::{ClientArgs, ClientCommands};
    pub use crate::core::{
        Config, Metrics, SearchQuery, SearchResult, Source, check_source_valid, get_source_path,
        load_config, save_config,
    };
    pub use crate::server::{get_active_source, list_databases, start_server};
    pub use crate::server_args::{ServerArgs, ServerCommands};
    pub use crate::utils::{EmbeddingStore, Ingestor, Provider, VectorData, log};
}
