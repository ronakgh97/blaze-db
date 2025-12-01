use anyhow::Result;
use blaze_db::prelude::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Commands::Init { path }) => {
            init_run(path).await?;
        }
        Some(Commands::Create { name, dimensions }) => {
            create_run(name, dimensions).await?;
        }
        Some(Commands::Serve { .. }) => {}
        Some(Commands::List { .. }) => {}

        None => todo!(),
    }

    Ok(())
}
