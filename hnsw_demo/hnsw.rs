use colored::Colorize;
use rand::Rng;
use rand::seq::SliceRandom;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use wide::f32x8;

#[derive(Debug, Clone)]
struct NSW {
    pub nodes: Vec<Node>,
    pub max_neighbours: usize,
}

impl NSW {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            max_neighbours: 16,
        }
    }

    /// Add a node to be rearranged later in bulk
    pub fn add_node_rearranged_later(&mut self, node: Node) {
        self.nodes.push(node);
    }

    #[allow(unused)]
    // Insert a node into the NSW graph, with incremental updates
    pub fn incremental_insert_node(&mut self, node: Node) {
        unimplemented!("INSERT THE NODE INTO THE GRAPH HERE");
    }

    // Rearrange all nodes in the graph after bulk insertion (slow method)
    // Returns the rearranged nodes, why not mut, cuz we need to for incremental insert later
    pub fn rearrange_nodes(&self) -> Vec<Node> {
        let mut nodes = self.nodes.clone();
        let mut rng = rand::rng();
        nodes.shuffle(&mut rng); // randomness

        let rearranged_nodes = nodes
            .par_iter()
            .map(|node| {
                // For each node, find its nearest neighbors and connect them
                let mut neighbors = Vec::new();

                // Search for nearest neighbors among all other nodes
                let mut results: Vec<(NodeIndex, f32)> = nodes
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| *idx != node.index) // Exclude self
                    .map(|(idx, other_node)| {
                        let score = cosine_similarity(&node.vector, &other_node.vector);
                        (idx, score)
                    })
                    .collect();

                // Sort results by similarity score in descending order
                results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                // Select top max_neighbors
                for (neighbor_idx, _) in results.into_iter().take(self.max_neighbours) {
                    neighbors.push(neighbor_idx);
                }

                // Create a new node with updated neighbors
                let rearranged_node = Node::new(node.index, node.vector, neighbors);
                rearranged_node
            })
            .collect::<Vec<Node>>();

        rearranged_nodes
    }
}

type NodeIndex = usize;

#[derive(Debug, Clone)]
struct Node {
    pub index: NodeIndex,
    pub vector: [f32; 1024],
    pub neighbors: Vec<NodeIndex>,
}

impl Node {
    pub fn new(index: NodeIndex, vector: [f32; 1024], neighbors: Vec<NodeIndex>) -> Self {
        Self {
            index,
            vector,
            neighbors,
        }
    }
}

fn main() {
    println!("\n === NSW DEMO === \n");

    let mut nsw = NSW::new();

    // Generate 100K random vectors
    let num_vectors = 20_000;
    for i in 0..num_vectors {
        let vector = generate_random_vector();

        // if (i + 1) % 10000 == 0 {
        //     println!("Generated {} vectors", (i + 1).to_string().cyan());
        // }

        // Create a node with none neighbors for simplicity
        let node = Node::new(i, vector, vec![]);
        nsw.add_node_rearranged_later(node);
    }

    // Rearrange nodes to build the graph with neighbors
    println!(
        "Building NSW graph with {} nodes...\n",
        nsw.nodes.len().to_string().cyan()
    );
    let start_time = std::time::Instant::now();
    let graph = nsw.rearrange_nodes();
    let duration = start_time.elapsed().as_secs_f64();
    println!("Rearranged in {}s", duration.to_string().yellow());

    // Analyze the graph
    graph_analyze(&graph, &nsw);
}

fn graph_analyze(nodes: &Vec<Node>, nsw: &NSW) {
    let total_nodes = nodes.len();
    let total_edges: usize = nodes.iter().map(|node| node.neighbors.len()).sum();
    let average_edges = total_edges as f32 / total_nodes as f32;

    // Find node(s) with the most neighbors
    let most_neighbors_node: Vec<&Node> = nodes
        .par_iter()
        .filter(|node| node.neighbors.len() == nsw.max_neighbours)
        .collect();

    println!("\nGraph Analysis:");
    println!("Total Nodes: {}", total_nodes);
    println!("Total Edges: {}", total_edges);
    println!("Average Edges per Node: {:.2}", average_edges);
    println!(
        "Nodes with most neighbour count: {:?}",
        most_neighbors_node.len()
    );
}

#[allow(unused)]
struct QueryResult {
    pub node: Node,
    pub similarity: f32,
}

#[allow(unused)]
/// Perform query on built NSW graph
fn query_nsw(vector: &[f32; 1024], top_k: i32, nodes: &Vec<Node>) -> Vec<QueryResult> {
    // Get a random start node
    let mut rng = rand::rng();
    let start_index = rng.random_range(0..nodes.len());
    let start_node = &nodes[start_index];

    unimplemented!("HElP ME SOMEONE 😞")
}

fn generate_random_vector() -> [f32; 1024] {
    let mut rng = rand::rng();

    let mut vector = [0.0f32; 1024];
    for i in 0..1024 {
        vector[i] = rng.random_range(0.0..1.0);
    }
    vector
}

/// Cosine similarity using 8-wide f32 vectors
/// Returns value in [-1, 1]
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
