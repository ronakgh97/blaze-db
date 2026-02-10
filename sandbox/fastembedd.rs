use anyhow::Result;

/// These bin is to use local inference and embed a model in our server
/// But Rust cpp bindings are too low level, so I am wait for a high level wrapper
#[tokio::main]
async fn main() -> Result<()> {
    Ok(())
}
