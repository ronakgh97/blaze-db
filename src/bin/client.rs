use anyhow::Result;
use blaze_db::prelude::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = ClientArgs::parse();
    match args.command {
        Some(ClientCommands::Config { url }) => {
            init_run_client(url).await?;
        }
        Some(ClientCommands::Create {
            source,
            name,
            dimensions,
        }) => {
            create_run(name, source, dimensions).await?; // TODO: source is unused (will be used in server uses multiple sources)
        }
        Some(ClientCommands::Embed {
            file,
            source,
            database,
            batch,
        }) => {
            embed_run(file, database, source, batch).await?;
        }
        Some(ClientCommands::Query {
            source,
            database,
            search,
            top_k,
        }) => {
            query_run(database, source, search, top_k).await?;
        }
        Some(ClientCommands::Ls) => {
            list_run().await?;
        }
        None => {
            let _ = print_ascii().await;
        }
    }

    Ok(())
}
