use crate::core::check_source_valid;
use crate::prelude::ServerConfig;
use crate::server::start_server;
use crate::{error, info};
use anyhow::Result;

pub async fn serve_run(port: Option<u16>, source: &String) -> Result<()> {
    println!("Starting the Server...");

    let config =
        ServerConfig::load_config(&ServerConfig::get_default_server_config_path()?).await?;

    let port = port.unwrap_or(config.server_connection.port);

    if check_source_valid(source).await? {
        info!("Source: {} is valid", &source);
        start_server(port, source.clone()).await;
        Ok(())
    } else {
        error!("Source: {} is not valid", &source);
        Err(anyhow::anyhow!("Source: {} is not valid", &source))
    }
}
