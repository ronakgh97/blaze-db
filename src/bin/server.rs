use anyhow::Result;
use blaze_db::prelude::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = ServerArgs::parse();
    match args.command {
        Some(ServerCommands::Init { .. }) => {
            init_run_server().await?;
        }

        Some(ServerCommands::Serve { port, source }) => {
            serve_run(port, source).await?;
        }
        None => {
            let _ = print_ascii().await;
        }
    }

    Ok(())
}
