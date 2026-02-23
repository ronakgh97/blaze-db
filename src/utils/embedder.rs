use anyhow::Result;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// A Bridge Wrapper struct to hold vector data and associated metadata, for outside module usage
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VectorData {
    pub chunk: Vec<String>,
    pub embedding: Vec<Vec<f32>>,
    pub dimensions: usize,
}

impl Default for VectorData {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorData {
    /// Create an empty VectorData
    pub fn new() -> Self {
        Self {
            chunk: Vec::new(),
            embedding: Vec::new(),
            dimensions: 0,
        }
    }

    #[inline]
    /// Get a specific vector by index
    pub fn get_vector(&self, index: usize) -> Option<&[f32]> {
        self.embedding.get(index).map(|v| v.as_slice())
    }

    #[inline]
    /// Get text chunk by index
    pub fn get_chunk(&self, index: usize) -> Option<&str> {
        self.chunk.get(index).map(|s| s.as_str())
    }

    #[inline]
    /// Calculate the raw data size in MB (vectors + metadata strings only)
    /// This represents the actual memory footprint of the data, not the serialized file size.
    pub fn data_size_mb(&self) -> f64 {
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

    /// Total number of embeddings
    pub fn len(&self) -> usize {
        self.embedding.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.embedding.is_empty()
    }

    // Read VectorData from disk JSON file (Memory mapped)
    pub async fn read_from_disk(path: &Path) -> Result<Self> {
        let path = path.to_path_buf();
        let json_data = tokio::task::spawn_blocking(move || -> Result<Self> {
            let file = std::fs::File::open(path)?;
            let mmap = unsafe { memmap2::Mmap::map(&file)? };
            let data_str = std::str::from_utf8(&mmap)?;
            let vector_data: VectorData = serde_json::from_str(data_str)?;
            Ok(vector_data)
        });
        json_data.await?
    }

    // Write VectorData to disk as a JSON file
    pub async fn write_to_disk(&self, path: &PathBuf) -> Result<()> {
        let json_data = serde_json::to_string_pretty(self)?;
        tokio::fs::write(path, json_data).await?;
        Ok(())
    }
}

/// Wrapper just for serialize external embedding API response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Embeddings {
    pub data: Vec<EmbeddingData>,
}

/// Wrapper just for serialize external embedding API response
#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub mock_mode: bool,
    pub mock_dimensions: usize,
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
            mock_mode: false,
            mock_dimensions: 1024,
        }
    }

    pub fn init_mock(dimensions: usize) -> Self {
        Self {
            url: "mock://localhost".to_string(),
            model: "mock-embeddings".to_string(),
            api_key: "mock-key".to_string(),
            client: reqwest::Client::new(),
            mock_mode: true,
            mock_dimensions: dimensions,
        }
    }

    /// Generate deterministic mock embeddings based on content hash
    /// Uses SHA256 hash of each chunk to create reproducible vectors
    fn generate_mock_embeddings(&self, chunks: &[String]) -> Result<VectorData> {
        let dimensions = self.mock_dimensions;
        let mut all_embeddings = Vec::with_capacity(chunks.len());

        for chunk in chunks {
            let mut hasher = Sha256::new();
            hasher.update(chunk.as_bytes());
            let hash_result = hasher.finalize();

            let mut vector = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let hash_byte = hash_result[i % hash_result.len()];
                // Convert to [-1, 1] range using simple normalization
                let value = (hash_byte as f32 / 127.5) - 1.0;
                vector.push(value);
            }

            all_embeddings.push(vector);
        }

        Ok(VectorData {
            chunk: chunks.to_vec(),
            embedding: all_embeddings,
            dimensions,
        })
    }

    #[inline]
    pub fn pretty_display(&self) -> String {
        format!(
            "Provider (Model: {}, Url: {}..., Key: {}...)",
            &self.model,
            &self.url[..12.min(self.url.len())],
            &self.api_key[..4.min(self.api_key.len())]
        )
    }

    /// Fetch embedding for a single piece of text
    pub async fn fetch_embedding(&self, text: &str) -> Result<VectorData> {
        self.fetch_embeddings(&[text.to_string()]).await
    }

    /// Fetch embeddings for the given chunks of text
    pub async fn fetch_embeddings(&self, chunks: &[String]) -> Result<VectorData> {
        if self.mock_mode {
            return self.generate_mock_embeddings(chunks);
        }

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
