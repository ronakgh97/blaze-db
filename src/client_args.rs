use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "blaze_db",
    version = "1.0.0-beta",
    about = "Blaze-DB: A high-performance vector database for embeddings",
    long_about = "A CLI client for managing Blaze-DB vector databases"
)]
pub struct ClientArgs {
    #[command(subcommand)]
    pub command: Option<ClientCommands>,
}

#[derive(Subcommand)]
pub enum ClientCommands {
    /// Initialize dotfiles and source dirs for Blaze-DB
    Init {
        /// Attempt to fix existing source_path issues
        #[arg(short, long)]
        fix: bool,
    },

    /// Initialize a new Data source
    New {
        /// Path to the new data source directory
        #[arg(short, long)]
        name: String,
    },

    /// Create a new Blaze-DB database
    Create {
        /// Source data path for the database
        #[arg(short, long)]
        source: String,

        /// Name of the new database
        #[arg(short, long)]
        name: String,

        /// Number of dimensions for the database embeddings
        #[arg(short, long)]
        dimensions: usize,
    },

    Embed {
        /// Path to the file containing data to embed
        #[arg(short, long)]
        file: PathBuf,

        /// Name of the target database to embed data into
        #[arg(short, long)]
        database: String,

        /// Optional batch size for embedding processing
        #[arg(short, long)]
        batch: Option<usize>,
    },

    /// List existing Blaze-DB databases
    Ls {
        /// Source data path to filter databases
        #[arg(short, long)]
        source: Option<String>,
    },
}
