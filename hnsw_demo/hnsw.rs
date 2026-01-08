use anyhow::Result;
use rayon::iter::ParallelIterator;
#[allow(unused)]
use rayon::prelude::{IntoParallelIterator, ParallelExtend};
use std::collections::HashMap;
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
        let r: f32 = rand::random();
        let level = (-r.ln() / self.distribution_bias).floor() as usize;
        level
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
    let hnsw = HNSW::new(16, 6, 1.7);

    let random_levels = Mutex::new(HashMap::new());

    (0..5000).into_par_iter().for_each(|i| {
        let level = hnsw.get_random_level();
        let mut levels = random_levels.lock().unwrap();
        levels.insert(i, level);
    });

    let levels = random_levels.lock().unwrap();
    let total = levels.len() as f32;
    let mut counts = HashMap::new();

    for level in levels.values() {
        *counts.entry(level).or_insert(0) += 1;
    }
    for (level, count) in counts.iter() {
        let percent = (*count as f32 / total) * 100.0;
        println!("Level {}: {:.2}%", level, percent);
    }

    // Assert that count for level 0 > level 1 > level 2 > ...
    let mut prev_count = None;
    let mut sorted_counts: Vec<_> = counts.iter().collect();
    sorted_counts.sort_by_key(|(level, _)| *level);
    for (_, count) in sorted_counts.iter() {
        if let Some(prev) = prev_count {
            assert!(
                prev > count,
                "Bad distribution: previous count {} is not greater than current count {}",
                prev,
                count
            );
        }
        prev_count = Some(count);
    }

    Ok(())
}
