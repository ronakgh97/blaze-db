use anyhow::Result;
#[allow(unused)]
use rayon::iter::ParallelIterator;
#[allow(unused)]
use rayon::prelude::{IntoParallelIterator, ParallelExtend};
use std::collections::HashMap;
#[allow(unused)]
use std::sync::Mutex;

/// Hierarchical Navigable Small World (HNSW) graph structure for approximate nearest neighbor search.

#[allow(unused)]
struct HNSW {
    pub nodes: Vec<Node>,
    pub entry_point: Option<NodeId>,
    pub max_layers: usize,
    // Degree of each node (max number of neighbors)
    pub max_neighbors: usize,
    // Controls the layer distribution of nodes (exponential distribution bias)
    pub distribution_bias: f32,
}

impl HNSW {
    /// Creates a new HNSW instance with specified parameters.
    pub fn new(max_neighbors: usize, max_layers: usize, distribution_bias: f32) -> Self {
        HNSW {
            nodes: Vec::new(),
            entry_point: None,
            max_layers,
            max_neighbors,
            distribution_bias,
        }
    }

    /// Generates a random level for a new node based on an exponential distribution.
    pub fn get_random_level(&self) -> usize {
        let r: f32 = rand::random(); // TODO: Will this return 0.0?, ln(0) is undefined
        let level = (-r.ln() / self.distribution_bias).floor() as usize;
        // Clamp to [0, max_layers - 1]
        level.min(self.max_layers - 1)
    }
}

/// Unique identifier for a node in the HNSW graph.
type NodeId = usize;

#[allow(unused)]
#[derive(Debug)]
struct Node {
    pub id: NodeId,
    pub vector: Vec<f32>,
    // Neighbors per layer, e.g neighbors[0] is the list of neighbors in layer 0
    pub neighbors: Vec<Vec<NodeId>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let hnsw = HNSW::new(16, 6, 0.8);

    let mut random_levels = HashMap::new();

    // Fill the histogram with random levels
    for _ in 0..10000 {
        let level = hnsw.get_random_level();
        *random_levels.entry(level).or_insert(0) += 1;
    }

    // Analyze the distribution
    let mut levels: Vec<usize> = random_levels.keys().cloned().collect();
    levels.sort();

    println!(
        "\nLevel Distribution Percentage with bias: {}\n",
        hnsw.distribution_bias
    );
    for level in &levels {
        let count = random_levels.get(&level).unwrap();
        let percentage = (*count as f32 / 10000.0) * 100.0;
        println!("Level {}: {:.2}%", level, percentage);
    }

    // Assert that higher levels are less frequent
    for level in 1..(&levels).len() {
        let lower_count = random_levels.get(&(level - 1)).unwrap_or(&0);
        let higher_count = random_levels.get(&level).unwrap_or(&0);
        assert!(
            lower_count >= higher_count,
            "Level {} has more nodes than Level {}",
            level - 1,
            level
        );
    }

    Ok(())
}
