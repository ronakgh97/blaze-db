mod args;
mod cli;
mod core;
mod server;
pub mod utils;

pub mod prelude {
    pub use crate::cli::{Args, Commands};
    pub use crate::core::{Metrics, SearchQuery, SearchResult, Source, Config, save_config, load_config};
    pub use crate::server::start_server;
    pub use crate::utils::{EmbeddingStore, Ingestor, Provider, VectorData};
    pub use crate::args::init_run;
}
