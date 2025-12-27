use crate::core::ClientConfig;
use crate::prelude::ServerConfig;
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
    let server_config =
        ServerConfig::load_config(&ServerConfig::get_default_server_config_path()?).await;

    match server_config {
        Ok(server_config) => {
            println!("Server Config\n\n {:?}\n\n", server_config)
        }
        Err(_) => {
            eprintln!("No server config found");
        }
    }

    let client_config =
        ClientConfig::load_config(&ClientConfig::get_default_user_config_path()?).await;

    match client_config {
        Ok(client_config) => {
            println!("Client Config\n\n {:?}\n\n", client_config)
        }
        Err(_) => {
            eprintln!("No client config found");
        }
    }
    println!("\nGithub: https://github.com/ronakgh97/blaze-db\n");

    Ok(())
}
