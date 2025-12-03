use anyhow::Result;
use blaze_db::prelude::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = ServerArgs::parse();
    match args.command {
        Some(ServerCommands::Serve { port, source }) => {
            serve_run(port, &source).await?;
        }
        None => {
            print_ascii().await;
        }
    }

    Ok(())
}
