use anyhow::Result;
use embellama::{EmbeddingEngine, EngineConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let config = EngineConfig::builder()
        .with_model_path("models/Qwen3-Embedding-0.6B-Q8_0.gguf")
        .with_model_name("qwen-embed")
        .with_use_mmap(true)
        .with_cache_disabled()
        .with_context_size(4096)
        .with_n_threads(16)
        .with_batch_size(1024)
        .with_use_gpu(true)
        .with_n_gpu_layers(999) // Use GPU for all layers
        .with_memory_limit_mb(1024 * 6) // 6GB
        .build()?;

    let engine = EmbeddingEngine::new(config)?;

    let embedding = engine.embed(Option::from("qwen-embed"), "Hello, world!")?;
    println!("Embedding dimension: {}", embedding.len());

    Ok(())
}
