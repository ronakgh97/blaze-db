use anyhow::Result;
use blaze_db::prelude::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = ServerArgs::parse();
    match args.command {
        None => {}
        Some(_) => {}
    }

    Ok(())
}
