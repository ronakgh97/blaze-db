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
        Some(Commands::Create { .. }) => {}
        Some(Commands::Serve { .. }) => {}
        Some(Commands::List { .. }) => {}

        None => todo!(),
    }

    Ok(())
}
