use crate::server::start_server;
use anyhow::Result;

pub async fn serve_run() -> Result<()> {
    println!("Starting the Server...");

    start_server().await;

    Ok(())
}
