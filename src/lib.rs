mod args;
mod client_args;
mod core;
mod server;
mod server_args;
pub mod utils;

pub mod prelude {
    pub use crate::args::{init_run, list_run, new_run, print_ascii, serve_run};
    pub use crate::client_args::{ClientArgs, ClientCommands};
    pub use crate::core::{
        Config, Metrics, SearchQuery, SearchResult, Source, load_config, save_config,
    };
    pub use crate::server::start_server;
    pub use crate::server_args::{ServerArgs, ServerCommands};
    pub use crate::utils::{EmbeddingStore, Ingestor, Provider, VectorData};
}
