use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "blaze_db",
    version = "1.0.0-beta",
    about = "Blaze-DB: A high-performance vector database for embeddings",
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

    /// Initialize a new Data source
    New {
        /// Name of the new source
        #[arg(short, long)]
        name: String,
    },

    /// Start the server
    Serve {
        /// Optional port to run the server on
        #[arg(short, long)]
        port: Option<u16>,

        /// Use specified source path or use default-src
        #[arg(short, long)]
        source: Option<String>,
    },
}
