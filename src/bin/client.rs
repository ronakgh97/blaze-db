use anyhow::Result;
use blaze_db::prelude::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = ClientArgs::parse();
    match args.command {
        Some(ClientCommands::Init { .. }) => {
            init_run().await?;
        }
        Some(ClientCommands::New { name }) => {
            new_run(name).await?;
        }
        Some(ClientCommands::Create {
            source: _,
            name,
            dimensions,
        }) => {
            create_run(name, dimensions).await?; // source is unused in this context
        }
        Some(ClientCommands::Embed {
            file,
            database,
            batch,
        }) => {
            embed_run(file, database, batch).await?;
        }
        Some(ClientCommands::Ls { source }) => {
            list_run(source).await?;
        }
        None => {
            print_ascii().await;
        }
    }

    Ok(())
}
