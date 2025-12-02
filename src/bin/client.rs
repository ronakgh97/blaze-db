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
        Some(ClientCommands::New { path }) => {
            new_run(path).await?;
        }

        Some(ClientCommands::Create { name, dimensions }) => {
            create_run(name, dimensions).await?;
        }
        Some(ClientCommands::List { .. }) => {}
        None => todo!(),
    }

    Ok(())
}
