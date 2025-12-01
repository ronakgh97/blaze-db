#[allow(unused)]
use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    name = "blaze-db",
    version = "1.0.0-beta",
    about = "Blaze-DB: A high-performance vector database for embeddings",
    long_about = "A CLI client for managing Blaze-DB vector databases"
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Blaze-DB repository
    Init {
        /// Initialize local Blaze-DB database at specified path
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Start the Blaze-DB server
    Serve {
        /// Port to run the server on (default: 8001)
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// List existing Blaze-DB databases
    List {
        /// List all local Blaze-DB databases
        #[arg(short, long)]
        db: Option<bool>,
    },

    /// Create a new Blaze-DB database
    Create {
        /// Name of the new database
        #[arg(short, long)]
        name: String,

        /// Number of dimensions for the database embeddings
        #[arg(short, long)]
        dimensions: usize,
    },
}
