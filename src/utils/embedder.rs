use anyhow::Result;
use bincode::{Decode, Encode};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Embeddings {
    pub data: Vec<EmbeddingData>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
pub struct EmbeddingData {
    pub index: usize,
    #[serde(default)]
    pub chunk: String,
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub dimensions: usize,
}

#[derive(Debug, Clone)]
pub struct Provider {
    pub url: String,
    pub model: String,
}

impl Provider {
    pub fn new(url: impl Into<String>, model: impl Into<String>) -> Self {
        let url = url.into();
        let model = model.into();
        if model.is_empty() {
            // Default model if none provided
            let default_model = "text-embedding-nomic-embed-text-v1.5";
            println!("Model not provided. Using default model: {}", default_model);
            return Self {
                url,
                model: default_model.to_string(),
            };
        }
        Self { url, model }
    }

    /// Fetch embedding for a single piece of text
    pub async fn fetch_embedding(&self, text: &str) -> Result<Embeddings> {
        self.fetch_embeddings(&[text.to_string()]).await
    }

    /// Fetch embeddings for the given chunks of text
    pub async fn fetch_embeddings(&self, chunks: &[String]) -> Result<Embeddings> {
        let body = serde_json::json!({
            "model": &self.model,
            "input": chunks,
        });

        let response = reqwest::Client::new()
            .post(&self.url)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch embeddings: HTTP {}", response.status());
        }

        let mut embeddings_response: Embeddings = response.json().await?;

        // Validate & filter embeddings
        embeddings_response.data = embeddings_response
            .data
            .into_par_iter()
            .filter(|embedding| !embedding.embedding.is_empty())
            .collect();

        // Fill in the chunk & dimensions for each embedding
        embeddings_response.data.iter_mut().for_each(|embedding| {
            if let Some(chunk) = chunks.get(embedding.index) {
                embedding.chunk = chunk.to_string();
            } else {
                embedding.chunk = String::from("");
            }

            embedding.dimensions = embedding.embedding.len();
        });

        Ok(embeddings_response)
    }
}
