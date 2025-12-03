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
    /// Start the server
    Serve {
        /// Optional port to run the server on
        #[arg(short, long)]
        port: Option<u16>,

        /// Use specified database path
        #[arg(short, long, required = true)]
        source: String,
    },
}
