use anyhow::Result;
use colored::Colorize;
use rand::Rng;
#[allow(unused)]
use rayon::iter::ParallelIterator;
#[allow(unused)]
use rayon::prelude::{IntoParallelIterator, ParallelExtend};
use std::collections::HashMap;
#[allow(unused)]
use std::sync::Mutex;
use wide::f32x8;

/// Hierarchical Navigable Small World (HNSW) graph structure for approximate nearest neighbor search.

#[allow(unused)]
struct HNSW {
    // All nodes in the graph, not layer-wise
    pub nodes: Vec<Node>,
    // First node at the top layer, used as entry point for searches
    pub entry_point: Option<NodeId>,
    // Total number of layers in the graph
    pub max_layers: usize,
    // Degree of each node (max number of neighbors)
    pub max_neighbors: usize,
    // Controls the layer distribution of nodes (exponential distribution bias) CURRENTLY UNUSED
    pub distribution_bias: f32,
}

impl HNSW {
    /// Creates a new HNSW instance with specified parameters.
    pub fn new(max_neighbors: usize, layers: usize, distribution_bias: f32) -> Self {
        HNSW {
            nodes: Vec::new(),
            entry_point: None,
            max_layers: layers,
            max_neighbors,
            distribution_bias, // Currently unused
        }
    }

    /// Generates a random level for a new node based on an exponential distribution.
    /// Uses the HNSW paper formula: floor(-ln(rand) * 1/ln(M))
    pub fn get_random_level(&self) -> usize {
        let r: f32 = rand::random::<f32>().max(1e-9);
        let m = 1.0 / (self.max_neighbors as f32).ln();
        let level = (-r.ln() * m).floor() as usize;
        level.min(self.max_layers - 1)

        // Alternative simpler version without precomputed bias
        // let r: f32 = rand::random();
        // let level = (-r.ln() / self.distribution_bias).floor() as usize;
        // // Clamp to [0, max_layers - 1]
        // level.min(self.max_layers - 1)
    }

    #[allow(unused)]
    pub fn add_node(&mut self, node: Node) {
        // Add the node to the graph
        self.nodes.push(node);
    }

    #[allow(unused)]
    // Builds the HNSW graph by connecting nodes based on their levels and distances.
    pub fn build_graph(&mut self) {}
}

/// Unique identifier for a node in the HNSW graph.
type NodeId = usize;

#[allow(unused)]
#[derive(Debug)]
// Represents a node in the HNSW graph.
struct Node {
    // Unique identifier for the node
    pub id: NodeId,
    // Vector representation of the node, any dimensionality
    pub vector: Vec<f32>,
    // Neighbors per layer, e.g neighbors[0] is the list of neighbors in layer 0
    pub neighbors: Vec<Vec<NodeId>>,
    // The highest layer this node exists in
    pub max_level: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut hnsw = HNSW::new(16, 6, 0.8);

    get_level_math_debug(&hnsw).await?;

    let node_count = 50000; // 50k nodes

    for i in 0..node_count {
        let node = Node {
            id: i,
            vector: generate_random_vector(128),
            neighbors: vec![Vec::new(); 6],
            max_level: hnsw.get_random_level(),
        };

        hnsw.add_node(node);
    }

    Ok(())
}

#[inline]
fn generate_random_vector(dimension: usize) -> Vec<f32> {
    let mut rng = rand::rng();

    let mut vector = vec![0.0f32; dimension];
    for i in 0..dimension {
        vector[i] = rng.random_range(-2.0..2.0);
    }
    vector
}

/// Cosine similarity using 8-wide f32 vectors
/// Returns value in [-1, 1]
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let chunks = a.len() / 8;
    let mut dot = f32x8::ZERO;
    let mut norm_a = f32x8::ZERO;
    let mut norm_b = f32x8::ZERO;

    // Process 8 elements at a time with SIMD
    for i in 0..chunks {
        let offset = i * 8;
        let va = f32x8::from(&a[offset..offset + 8]);
        let vb = f32x8::from(&b[offset..offset + 8]);
        dot += va * vb;
        norm_a += va * va;
        norm_b += vb * vb;
    }

    // Reduce SIMD vectors to scalars
    let arr_dot = dot.to_array();
    let arr_na = norm_a.to_array();
    let arr_nb = norm_b.to_array();

    let mut dot_sum: f32 = arr_dot.iter().sum();
    let mut na_sum: f32 = arr_na.iter().sum();
    let mut nb_sum: f32 = arr_nb.iter().sum();

    // Handle remaining elements (tail)
    let remainder_start = chunks * 8;
    for i in remainder_start..a.len() {
        dot_sum += a[i] * b[i];
        na_sum += a[i] * a[i];
        nb_sum += b[i] * b[i];
    }

    let denominator = (na_sum * nb_sum).sqrt();
    if denominator < f32::EPSILON {
        0.0
    } else {
        dot_sum / denominator
    }
}

#[allow(unused)]
async fn get_level_math_debug(hnsw: &HNSW) -> Result<()> {
    let mut random_levels = HashMap::new();

    let samples = 1_000_000;

    for _ in 0..samples {
        let level = hnsw.get_random_level();
        *random_levels.entry(level).or_insert(0) += 1;
    }

    // Analyze the distribution
    let mut levels: Vec<usize> = random_levels.keys().cloned().collect();
    levels.sort();

    println!(
        "\nLevel Distribution (M={}, m={:.3}) with {} samples:\n",
        hnsw.max_neighbors,
        1.0 / (hnsw.max_neighbors as f32).ln(),
        samples
    );

    println!("Level | Count     | Percentage | Expected ~1/M ratio");
    println!("------|-----------|------------|--------------------");

    for level in &levels {
        let count = random_levels.get(&level).unwrap();
        let percentage = (*count as f32 / samples as f32) * 100.0;

        let expected_ratio = if *level > 0 {
            let prev_count = random_levels.get(&(level - 1)).unwrap_or(&1);
            let ratio = (*count as f32 / *prev_count as f32) * 100.0;
            format!("{:.2}%", ratio)
        } else {
            "N/A".to_string()
        };

        println!(
            "{:5} | {:9} | {:10.4} | {}",
            level.to_string().yellow(),
            count.to_string().cyan(),
            percentage.to_string().cyan(),
            expected_ratio.to_string().green()
        );

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
    }

    Ok(())
}
