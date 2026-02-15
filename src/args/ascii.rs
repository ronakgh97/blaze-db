use crate::core::{SERVER_FILE, UserConfig};
use crate::prelude::{Source, VectorBase};
use colored::Colorize;

pub async fn print_ascii() -> anyhow::Result<()> {
    let ascii_art = r#"
$$\       $$\                                           $$\ $$\
$$ |      $$ |                                          $$ |$$ |      
$$$$$$$\  $$ | $$$$$$\  $$$$$$$$\  $$$$$$\         $$$$$$$ |$$$$$$$\  
$$  __$$\ $$ | \____$$\ \____$$  |$$  __$$\       $$  __$$ |$$  __$$\ 
$$ |  $$ |$$ | $$$$$$$ |  $$$$ _/ $$$$$$$$ |      $$ /  $$ |$$ |  $$ |
$$ |  $$ |$$ |$$  __$$ | $$  _/   $$   ____|      $$ |  $$ |$$ |  $$ |
$$$$$$$  |$$ |\$$$$$$$ |$$$$$$$$\ \$$$$$$$\       \$$$$$$$ |$$$$$$$  |
\_______/ \__| \_______|\________| \_______|       \_______|\_______/                                                                                                                                            
"#;

    println!("{}\n", ascii_art.to_string().yellow());

    // Display server configuration
    let server_file = SERVER_FILE.read().await;
    match server_file.get_all_sources() {
        Ok(sources) => {
            let total_sources = sources.len();
            let total_vector_bases: usize = sources.iter().map(|s| s.vector_bases.len()).sum();
            let total_nodes: u32 = sources
                .iter()
                .flat_map(|s| s.vector_bases.iter())
                .map(|vb| vb.node_count)
                .sum();
            println!(" Server Configuration:");
            println!("  Total sources: {}", total_sources.to_string().cyan());
            println!(
                "  Total vector bases: {}",
                total_vector_bases.to_string().cyan()
            );
            println!("  Total nodes: {}", total_nodes.to_string().yellow());

            // List sources
            if let Ok(sources) = server_file.get_all_sources() {
                let sources: Vec<Source> = sources.into_iter().take(5).collect();

                for src in sources {
                    println!("   • Source: {}", src.source_name.green());
                    let vector_bases: Vec<VectorBase> =
                        src.vector_bases.clone().into_iter().take(5).collect();
                    if !vector_bases.is_empty() {
                        for vb in vector_bases {
                            println!("      - Database: {}", vb.vb_name.cyan());
                        }
                        let remaining_bases = src.vector_bases.clone().len().saturating_sub(5);
                        if remaining_bases > 0 {
                            println!(
                                "      ... and {} more databases",
                                remaining_bases.to_string().yellow()
                            );
                        }
                    } else {
                        println!("      - No databases found");
                    }
                }
                let remaining_sources = total_sources.saturating_sub(5);
                if remaining_sources > 0 {
                    println!(
                        "   ... and {} more sources",
                        remaining_sources.to_string().yellow()
                    );
                }
            }
            println!();
        }
        Err(_) => {
            eprintln!(" No server data found");
        }
    }

    // Display client configuration
    let client_config = UserConfig::load_config(&UserConfig::get_default_path()?).await;

    match client_config {
        Ok(client_config) => {
            println!(" Client Configuration\n");
            println!(" User: {}", client_config.user.username);
            dotenv::dotenv().ok();
            if let Ok(api_key) = std::env::var("BLAZE_API_KEY") {
                println!(" API Key: {}****", api_key[0..6].to_string().dimmed());
            }

            println!(" Service URL: {}", client_config.server.server_url);
            println!(
                "  Instance Server URL: {}",
                client_config.server.instance_url
            );
            println!();
        }
        Err(_) => {
            eprintln!(" No client config found\n");
        }
    }

    println!(
        "🔗  Github: {}",
        "https://github.com/ronakgh97/blaze-db\n".blue().bold()
    );

    Ok(())
}
