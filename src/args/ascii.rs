use crate::core::{ClientConfig, SERVER_FILE};
use colored::Colorize;

pub async fn print_ascii() -> anyhow::Result<()> {
    let ascii_art = r#"
    ▄▄    ▄▄                        ▄▄ ▄▄
    ██    ██                        ██ ██
    ████▄ ██  ▀▀█▄ ▀▀▀██ ▄█▀█▄   ▄████ ████▄
    ██ ██ ██ ▄█▀██   ▄█▀ ██▄█▀   ██ ██ ██ ██
    ████▀ ██ ▀█▄██ ▄██▄▄ ▀█▄▄▄   ▀████ ████▀
    "#;

    println!("{}\n\n", ascii_art.to_string().yellow());

    // Display server configuration
    let server_file = SERVER_FILE.read().await;
    match server_file.stats() {
        Ok(stats) => {
            println!("  Total sources: {}", stats.total_sources);
            println!("  Total vector bases: {}", stats.total_vector_bases);
            println!("  Total nodes: {}", stats.total_nodes);

            // List sources
            if let Ok(sources) = server_file.list_sources() {
                println!("\n Sources: {:?}", sources);
            }
            println!();
        }
        Err(_) => {
            eprintln!(" No server data found");
        }
    }

    // Display client configuration
    let client_config =
        ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await;

    match client_config {
        Ok(client_config) => {
            println!(" Client Configuration:");
            println!("  URL: {}", client_config.url);
            println!("  Timeout: {}s", client_config.timeout);
            println!();
        }
        Err(_) => {
            eprintln!(" No client config found\n");
        }
    }

    println!("🔗 Github: https://github.com/ronakgh97/blaze-db\n");

    Ok(())
}
