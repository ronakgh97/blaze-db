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

        Some(ServerCommands::Serve {
            port,
            backup,
            no_env,
            source,
        }) => {
            serve_run(port, backup, no_env, source).await?;
        }
        Some(ServerCommands::Bench { .. }) => {
            bench_run().await?;
        }
        Some(ServerCommands::Sync { .. }) => {
            //    sync_run().await?;
        }
        None => {
            let _ = print_ascii().await;
        }
    }

    Ok(())
}
