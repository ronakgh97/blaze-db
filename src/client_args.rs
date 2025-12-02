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
        path: PathBuf,
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

    /// List existing Blaze-DB databases
    List {
        /// List all local Blaze-DB databases
        #[arg(short, long)]
        db: Option<bool>,
    },
}
