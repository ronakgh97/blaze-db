use crate::core::load_config;
use anyhow::Result;

pub async fn list_run(source: Option<String>) -> Result<()> {
    println!("Listing all managed source dirs...");

    let config = load_config().await?;

    let source_list = config.data_source.source_name;

    match source {
        Some(src) => {
            if let Some(sources) = source_list {
                if sources.contains(&src) {
                    println!("Source '{}' is present.", src);
                } else {
                    println!("Source '{}' is not present.", src);
                }
            } else {
                println!("No sources are currently present.");
            }
        }
        None => {
            if let Some(sources) = source_list {
                println!("Available sources:");
                for src in sources {
                    println!("- {}", src);
                }
            } else {
                println!("No sources are currently present.");
            }
        }
    }

    Ok(())
}
