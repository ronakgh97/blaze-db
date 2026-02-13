use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "blaze_db",
    version = "1.0.0-beta",
    about = "Blaze-DB: A high-performance vector database",
    long_about = "A CLI server for managing Blaze-DB vector databases"
)]
pub struct ServerArgs {
    #[command(subcommand)]
    pub command: Option<ServerCommands>,
}
#[derive(Subcommand)]
pub enum ServerCommands {
    /// Initialize the Blaze-DB server source directory
    Init {
        /// Attempt to fix existing source_path issues
        #[arg(short, long)]
        fix: bool,
    },

    /// Start the server
    Serve {
        /// Optional port to run the server on
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable automatic backups on server startup
        #[arg(short, long)]
        backup: bool,

        /// Use specified source path or use all [sources] if not provided, during server startup
        #[arg(short, long)]
        source: Option<Vec<String>>, //TODO: Will use this later...
    },

    /// Run benchmarks
    Bench {
        /// Test HNSW index performance (TODO: Add more benchmark options later)
        #[arg(short, long)]
        index: bool,

        /// Test Concurrency model performance (TODO: Add more benchmark options later)
        #[arg(short, long)]
        concurrency: bool,
    },
}
