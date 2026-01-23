use anyhow::Result;
use bincode::{Decode, Encode};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

// A Bridge Wrapper struct to hold vector data and associated metadata, for outside module usage
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VectorData {
    pub chunk: Vec<String>,
    pub embedding: Vec<Vec<f32>>,
    pub dimensions: usize,
}

impl VectorData {
    /// Get a specific vector by index
    pub fn get_vector(&self, index: usize) -> Option<&[f32]> {
        self.embedding.get(index).map(|v| v.as_slice())
    }

    /// Get text chunk by index
    pub fn get_chunk(&self, index: usize) -> Option<&str> {
        self.chunk.get(index).map(|s| s.as_str())
    }

    /// Memory usage estimate in MB
    pub fn memory_usage_mb(&self) -> f64 {
        let vector_bytes: usize = self
            .embedding
            .par_iter()
            .map(|emb| emb.len() * size_of::<f32>())
            .sum();
        let metadata_bytes: usize = self
            .chunk
            .par_iter()
            .map(|c| c.len() * size_of::<u8>())
            .sum();
        (vector_bytes + metadata_bytes) as f64 / (1024.0 * 1024.0)
    }
}

/// Wrapper just for serialize external embedding API response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Embeddings {
    pub data: Vec<EmbeddingData>,
}

/// Wrapper just for serialize external embedding API response
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
    pub api_key: String,
    pub client: reqwest::Client,
}

impl Provider {
    pub fn init(
        url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        let url = url.into();
        let model = model.into();
        let api_key = api_key.into();
        Self {
            url,
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch embedding for a single piece of text
    pub async fn fetch_embedding(&self, text: &str) -> Result<VectorData> {
        self.fetch_embeddings(&[text.to_string()]).await
    }

    /// Fetch embeddings for the given chunks of text
    pub async fn fetch_embeddings(&self, chunks: &[String]) -> Result<VectorData> {
        let body = serde_json::json!({
            "model": &self.model,
            "input": chunks,
        });

        let response = self
            .client
            .post(&self.url)
            .json(&body)
            .header("Authorization", format!("Bearer {}", &self.api_key))
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

        // Map Embeddings to VectorData
        let all_chunks: Vec<String> = embeddings_response
            .data
            .iter()
            .map(|e| e.chunk.clone())
            .collect();
        let all_embeddings: Vec<Vec<f32>> = embeddings_response
            .data
            .iter()
            .map(|e| e.embedding.clone())
            .collect();
        let dimensions = all_embeddings.first().map(|v| v.len()).unwrap_or(0);
        Ok(VectorData {
            chunk: all_chunks,
            embedding: all_embeddings,
            dimensions,
        })
    }
}
