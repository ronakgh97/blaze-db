use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "blaze_db",
    version = "1.0.0-beta",
    about = "Blaze-DB: A high-performance vector database",
    long_about = "A CLI client wrapper for querying Blaze-DB vector database servers"
)]
pub struct ClientArgs {
    #[command(subcommand)]
    pub command: Option<ClientCommands>,
}

#[derive(Subcommand)]
pub enum ClientCommands {
    /// Initialize the Blaze-DB client configuration
    Config {
        /// Server URL
        #[arg(short, long)]
        url: Option<String>,
    },

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
        #[arg(short, long, default_value_t = 5)]
        top_k: usize,
    },

    /// List existing Blaze-DB databases from the all server source
    Ls,
}
