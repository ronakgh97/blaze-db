#![allow(unused)]
use blaze_db::prelude::{EmbeddingStore, Provider};
use blaze_db::utils::VectorData;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use rand::seq::SliceRandom;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::ParallelIterator;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator};
use wide::f32x8;

/// Navigable Small World (NSW) graph structure for approximate nearest neighbor search.
#[derive(Debug, Clone)]
struct NSW {
    pub nodes: Vec<Node>,
    pub max_neighbours: usize,
}

impl NSW {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            max_neighbours: 32,
        }
    }

    /// Add a node to be rearranged later in bulk
    pub fn add_node_rearranged_later(&mut self, node: Node) {
        self.nodes.push(node);
    }

    #[allow(unused)]
    // Insert a node into the NSW graph, with incremental updates
    pub fn incremental_insert_node(&mut self, node: Node) -> Vec<Node> {
        unimplemented!("Incremental insertion not implemented yet");
    }

    // Rearrange all nodes in the graph after bulk insertion (slow method)
    // Returns the rearranged nodes, why not mut, cuz we need to for incremental insert later
    pub fn rearrange_nodes(&self) -> Vec<Node> {
        // Progress bar setup
        let progress_bar = ProgressBar::new(self.nodes.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("●●-"),
        );

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
                results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                // Select top max_neighbors
                for (neighbor_idx, _) in results.into_iter().take(self.max_neighbours) {
                    neighbors.push(neighbor_idx);
                }

                // Create a new node with updated neighbors
                let rearranged_node = Node::new(node.index, node.vector.clone(), neighbors);
                progress_bar.inc(1);
                rearranged_node
            })
            .collect::<Vec<Node>>();

        rearranged_nodes
    }
}

type NodeIndex = usize;

/// Represents a node in the NSW graph.
#[derive(Debug, Clone)]
struct Node {
    pub index: NodeIndex,
    pub vector: Vec<f32>,
    pub neighbors: Vec<NodeIndex>,
}

impl Node {
    pub fn new(index: NodeIndex, vector: Vec<f32>, neighbors: Vec<NodeIndex>) -> Self {
        Self {
            index,
            vector,
            neighbors,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("\n NSW DEMO \n");

    let mut nsw = NSW::new();

    // Generate 50K random vectors
    let num_vectors = 50_000;

    for i in 0..num_vectors {
        let vector = generate_random_vector(1024);

        // if (i + 1) % 10000 == 0 {
        //     println!("Generated {} vectors", (i + 1).to_string().cyan());
        // }

        // Create a node with none neighbors for simplicity
        let node = Node::new(i, vector, vec![]);
        nsw.add_node_rearranged_later(node);
    }

    // Load vector from sample embeddings
    // let embeddings = load_vector_from_sample().await;
    //
    // for (i, embedding) in embeddings.embedding.iter().enumerate() {
    //     let mut vector = vec![0.0f32; embeddings.dimensions];
    //     for j in 0..embeddings.dimensions {
    //         vector[j] = embedding[j];
    //     }
    //     let node = Node::new(i, vector, vec![]);
    //     nsw.add_node_rearranged_later(node);
    // }

    // Rearrange nodes to build the graph with neighbors
    println!(
        "Building NSW graph with {} nodes...",
        nsw.nodes.len().to_string().cyan()
    );
    let start_time = std::time::Instant::now();
    let graph = nsw.rearrange_nodes();
    let duration = start_time.elapsed().as_secs_f64();
    println!("Rearranged in {}s", duration.to_string().yellow());

    // Analyze the graph
    //graph_analyze(&graph, &nsw);

    // Perform a query
    //let provider = Provider::new(
    //    "http://localhost:1234/v1/embeddings",
    //    "text-embedding-qwen3-embedding-0.6b",
    //);
    // let sample_query = "What is this book about?";
    //let query_embedding = provider.fetch_embedding(sample_query).await?;

    // let query_vector = query_embedding.data[0].embedding.clone();
    let query_vector = generate_random_vector(1024);
    println!("\nQuerying vector: {:?}...", &query_vector[..3]);
    let top_k = 5;

    // Greedy Search
    let start_time = std::time::Instant::now();
    let results = greedy_search(&query_vector, top_k, &graph);
    let duration = start_time.elapsed().as_secs_f64();
    println!(
        "\nGreedy search completed in {}s",
        duration.to_string().yellow()
    );
    println!("\nTop {} Greedy Search Results:", top_k);
    for (i, result) in results.iter().enumerate() {
        println!(
            "Result {}: Node Index: {}, Similarity: {:.4}",
            i + 1,
            result.node.index.to_string().cyan(),
            result.similarity.to_string().cyan()
        );
    }

    // Parallel Greedy Search
    let start_time = std::time::Instant::now();
    let start_points = 5;
    let parallel_results = parallel_greedy_search(&query_vector, top_k, start_points, &graph);
    let duration = start_time.elapsed().as_secs_f64();
    println!(
        "\nParallel Greedy search with {} start points, completed in {}s",
        start_points.to_string().yellow(),
        duration.to_string().yellow()
    );
    println!("\nTop {} Parallel Greedy Search Results:", top_k);
    for (i, result) in parallel_results.iter().enumerate() {
        println!(
            "Result {}: Node Index: {}, Similarity: {:.4}",
            i + 1,
            result.node.index.to_string().cyan(),
            result.similarity.to_string().cyan()
        );
    }

    // Brute-force Search
    let start_time = std::time::Instant::now();
    let brute_results = brute_search(&query_vector, top_k, &graph);
    let duration = start_time.elapsed().as_secs_f64();
    println!(
        "\nBrute search completed in {}s",
        duration.to_string().yellow()
    );
    println!("\nTop {} Brute-force Results:", top_k);
    for (i, result) in brute_results.iter().enumerate() {
        println!(
            "Result {}: Node Index: {}, Similarity: {:.4}",
            i + 1,
            result.node.index.to_string().cyan(),
            result.similarity.to_string().cyan()
        );
    }

    Ok(())
}

#[allow(unused)]
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
        "Nodes with most neighbour count: {:?}\n",
        most_neighbors_node.len()
    );
}

#[allow(unused)]
async fn load_vector_from_sample() -> VectorData {
    let binary_data = EmbeddingStore::read_binary("./embeddings")
        .await
        .expect("Failed to load embeddings");

    binary_data
}

#[derive(Debug, Clone)]
struct QueryResult {
    pub node: Node,
    pub similarity: f32,
}

/// Perform greedy search on built NSW graph
fn greedy_search(vector: &Vec<f32>, top_k: i32, nodes: &Vec<Node>) -> Vec<QueryResult> {
    // Get a random start node
    let mut rng = rand::rng();
    let start_index = rng.random_range(0..nodes.len());
    let mut start_node = &nodes[start_index];

    let mut result_buffer = Vec::new();

    loop {
        // Calculate similarity with the start node
        let similarity = cosine_similarity(vector, &start_node.vector);
        result_buffer.push(QueryResult {
            node: start_node.clone(),
            similarity,
        });

        // Find the best neighbor to continue the search
        let best_neighbor = start_node
            .neighbors
            .iter()
            .map(|&neighbor_idx| &nodes[neighbor_idx])
            .max_by(|a, b| {
                let sim_a = cosine_similarity(vector, &a.vector);
                let sim_b = cosine_similarity(vector, &b.vector);
                sim_a.partial_cmp(&sim_b).unwrap()
            });

        match best_neighbor {
            Some(neighbor) => {
                let neighbor_similarity = cosine_similarity(vector, &neighbor.vector);
                // If the best neighbor is better than the current node, move to it
                if neighbor_similarity > similarity {
                    start_node = neighbor;
                } else {
                    // No better neighbor found, end search
                    break;
                }
            }
            None => break, // No neighbors, end search
        }

        if result_buffer.len() >= top_k as usize {
            break;
        }
    }

    result_buffer.reverse();
    result_buffer
}

fn parallel_greedy_search(
    vector: &Vec<f32>,
    top_k: i32,
    start_points: usize,
    nodes: &Vec<Node>,
) -> Vec<QueryResult> {
    // Get multiple random start nodes
    let mut rng = rand::rng();
    let start_indices: Vec<usize> = (0..start_points)
        .map(|_| rng.random_range(0..nodes.len()))
        .collect();

    let mut result_buffer: Vec<QueryResult> = start_indices
        .par_iter()
        .map(|&start_index| {
            let mut start_node = &nodes[start_index];
            loop {
                // Calculate similarity with the start node
                let similarity = cosine_similarity(vector, &start_node.vector);

                // Find the best neighbor to continue the search
                let best_neighbor = start_node
                    .neighbors
                    .iter()
                    .map(|&neighbor_idx| &nodes[neighbor_idx])
                    .max_by(|a, b| {
                        let sim_a = cosine_similarity(vector, &a.vector);
                        let sim_b = cosine_similarity(vector, &b.vector);
                        sim_a.partial_cmp(&sim_b).unwrap()
                    });

                match best_neighbor {
                    Some(neighbor) => {
                        let neighbor_similarity = cosine_similarity(vector, &neighbor.vector);
                        // If the best neighbor is better than the current node, move to it
                        if neighbor_similarity > similarity {
                            start_node = neighbor;
                        } else {
                            // No better neighbor found, end search
                            break;
                        }
                    }
                    None => break, // No neighbors, end search
                }
            }

            QueryResult {
                node: start_node.clone(),
                similarity: cosine_similarity(vector, &start_node.vector),
            }
        })
        .collect();

    // Sort results by similarity in descending order
    result_buffer.sort_unstable_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());

    // Return top_k results
    result_buffer.into_par_iter().take(top_k as usize).collect()
}

/// Perform brute-force search for comparison and validation
fn brute_search(vector: &Vec<f32>, top_k: i32, nodes: &Vec<Node>) -> Vec<QueryResult> {
    let mut results: Vec<QueryResult> = nodes
        .par_iter()
        .map(|node| {
            let similarity = cosine_similarity(vector, &node.vector);
            QueryResult {
                node: node.clone(),
                similarity,
            }
        })
        .collect();

    // Sort results by similarity in descending order
    results.sort_unstable_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());

    // Return top_k results
    results.into_par_iter().take(top_k as usize).collect()
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

/// Generate a random vector of 1024 dimensions with values in range [-2.0, 2.0]
#[allow(unused)]
fn generate_random_vector(dimension: usize) -> Vec<f32> {
    let mut rng = rand::rng();

    let mut vector = vec![0.0f32; dimension];
    for i in 0..dimension {
        vector[i] = rng.random_range(-2.0..2.0);
    }
    vector
}
