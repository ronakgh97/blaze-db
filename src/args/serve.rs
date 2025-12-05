use crate::core::check_source_valid;
use crate::prelude::log;
use crate::server::start_server;
use crate::{error, info};
use anyhow::Result;

pub async fn serve_run(port: Option<u16>, source: &String) -> Result<()> {
    println!("Starting the Server...");

    let port = port.unwrap_or(8001);

    if check_source_valid(&source).await? {
        info!("Source: {} is valid", &source);
        start_server(port, source.clone()).await;
        Ok(())
    } else {
        error!("Source: {} is not valid", &source);
        Ok(())
    }
}
