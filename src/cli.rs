use crate::prelude::Metrics;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "blaze_db",
    version = "1.0.0-beta",
    about = "Blaze-DB: A high-performance vector database",
    long_about = "A CLI client wrapper for querying Blaze-DB servers"
)]
pub struct ClientArgs {
    #[command(subcommand)]
    pub command: Option<ClientCommands>,
}

#[derive(Subcommand)]
pub enum ClientCommands {
    /// Initialize the Blaze-DB client configuration
    Init {},

    /// Create a new database or a new source in Blaze-DB
    Create {
        /// Source name where the database will be stored
        #[arg(short, long, required = true)]
        source: String,

        /// Name of the new database
        #[arg(short, long)]
        name: Option<String>,

        /// Number of dimensions for the database embeddings
        #[arg(short, long)]
        dimensions: Option<usize>,

        /// Similarity metric to use for the database (e.g., COSINE, EUCLIDEAN, DOT_PRODUCT)
        #[arg(short, long)]
        metrics: Option<Metrics>,
    },

    /// Embed data into an existing Blaze-DB database
    Embed {
        /// Path to the file containing data to embed
        #[arg(short, long)]
        file: PathBuf,

        /// Source name where the database will be stored
        #[arg(short, long)]
        source: String,

        /// Name of the target database to embed data into
        #[arg(short, long)]
        database: String,

        /// Optional batch size for embedding processing
        #[arg(short, long)]
        batch: Option<usize>,
    },

    /// Query an existing Blaze-DB database for similar embeddings
    Query {
        /// Source name where the database will be stored
        #[arg(short, long)]
        source: String,

        /// Name of the database to query
        #[arg(short, long)]
        database: String,

        /// The query text to search for similar embeddings
        #[arg(short, long)]
        search: String,

        /// Number of top similar results to return
        #[arg(short, long, default_value_t = 10)]
        top_k: usize,
    },

    /// List existing Blaze-DB databases from the all server source
    Ls,
}

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

    /// Sync Catalog file with Indexes and Databases on disk (useful for recovery or after manual file changes)
    Sync {},

    /// Start the server
    Serve {
        /// Optional port to run the server on
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable automatic backups on server startup
        #[arg(short, long)]
        backup: bool,

        /// Disable loading environment variables from .env file on server startup
        /// (Good for benchmarking or dry run test)
        #[arg(short, long)]
        no_env: bool,

        /// Start server in sandbox mode with in-memory storage (no persistence, good for quick run or feature testing)
        #[arg(short, long)]
        sandbox: bool,

        /// Use specified source path or use all [sources] if not provided, during server startup
        #[arg(short, long)]
        source: Option<Vec<String>>, //TODO: Will use this later...
    },

    /// Run benchmarks in Isolated environment
    Bench {
        // /// Test HNSW index performance (TODO: Add more benchmark options later)
        // #[arg(short, long)]
        // index: bool,
        //
        // /// Test Concurrency model performance (TODO: Add more benchmark options later)
        // #[arg(short, long)]
        // concurrency: bool,
    },
}
