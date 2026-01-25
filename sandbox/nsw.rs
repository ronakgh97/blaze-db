mod utils;

use crate::utils::cosine_similarity;
#[allow(unused)]
use crate::utils::{generate_random_vector, load_sample_hnsw_index};
#[allow(unused)]
use blaze_db::prelude::{EmbeddingStore, Provider};
#[allow(unused)]
use blaze_db::utils::VectorData;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use rand::seq::SliceRandom;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::ParallelIterator;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator};
use std::collections::HashSet;

/// Navigable Small World (NSW) graph structure for approximate nearest neighbor search.
#[derive(Debug, Clone)]
struct NSW {
    pub nodes: Vec<Node>,
    pub max_neighbours: usize,
}

impl NSW {
    pub fn new(max_neighbours: usize, max_nodes: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(max_nodes), // Pre-allocate for efficiency
            max_neighbours,
        }
    }

    /// Add a node to be rearranged later in bulk
    pub fn add_node_index_later(&mut self, node: Node) {
        self.nodes.push(node);
    }

    #[allow(unused)]
    // Insert a node into the NSW graph, with incremental updates
    pub fn incremental_insert_node(&mut self, node: Node) -> Vec<Node> {
        unimplemented!("Incremental insertion not implemented yet");
    }

    // Rearrange all nodes in the graph after bulk insertion (slow method)
    // Returns the rearranged nodes, why not mut, cuz we need to for incremental insert later
    pub fn index_nodes(&self) -> Vec<Node> {
        // Progress bar setup
        let progress_bar = ProgressBar::new(self.nodes.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("●●>-"),
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
                let rearranged_node = Node::new(
                    node.index,
                    node.vector.clone(),
                    node.metadata.clone(),
                    neighbors,
                );
                progress_bar.inc(1);
                rearranged_node
            })
            .collect::<Vec<Node>>();

        rearranged_nodes
    }

    // Search API - Parallel Greedy Search
    // Starts from multiple random entry points and performs greedy search in parallel
    #[inline]
    pub fn parallel_greedy_search(
        vector: &Vec<f32>,
        top_k: i32,
        start_points: usize,
        nodes: &Vec<Node>,
    ) -> Vec<Node> {
        // Get multiple random start nodes
        let mut rng = rand::rng();
        let start_indices: Vec<usize> = (0..start_points)
            .map(|_| rng.random_range(0..nodes.len()))
            .collect();

        // Run parallel greedy searches from each start point
        // Collect ALL visited nodes, not just final destination (Important)
        let all_candidates: Vec<(Node, f32)> = start_indices
            .par_iter()
            .flat_map(|&start_index| {
                let mut current_node = &nodes[start_index];
                let mut path_results = Vec::new();

                loop {
                    // Calculate similarity with current node
                    let current_similarity = cosine_similarity(vector, &current_node.vector);
                    path_results.push((current_node.clone(), current_similarity));

                    // Find the best neighbor
                    let best_neighbor = current_node
                        .neighbors
                        .par_iter()
                        .map(|&neighbor_idx| {
                            let neighbor = &nodes[neighbor_idx];
                            (neighbor, cosine_similarity(vector, &neighbor.vector))
                        })
                        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                    match best_neighbor {
                        Some((neighbor, neighbor_similarity)) => {
                            // If neighbor is better, move to it
                            if neighbor_similarity > current_similarity {
                                current_node = neighbor;
                            } else {
                                // Found local optimum
                                break;
                            }
                        }
                        None => break, // No neighbors
                    }
                }

                path_results
            })
            .collect();

        // Sort all candidates by similarity descending
        let mut sorted_candidates = all_candidates;
        sorted_candidates.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Deduplicate and return top_k
        let mut seen = HashSet::new();
        sorted_candidates
            .into_iter()
            .filter(|(node, _)| seen.insert(node.index))
            .take(top_k as usize)
            .map(|(node, _)| node)
            .collect()
    }

    /// Search API - Brute-force Search
    /// Perform parallel brute-force search over all nodes
    /// Use a lot of cpu and memory, but accurate and slow
    #[inline]
    fn brute_search(vector: &Vec<f32>, top_k: i32, nodes: &Vec<Node>) -> Vec<Node> {
        let mut results: Vec<(Node, f32)> = Vec::new();
        results.reserve(nodes.len()); // Pre-allocate

        results = nodes
            .par_iter()
            .map(|node| {
                let similarity = cosine_similarity(vector, &node.vector);
                (node.clone(), similarity)
            })
            .collect();

        // Sort results by similarity in descending order
        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Return top_k results as Vec<Node>
        results
            .into_par_iter()
            .take(top_k as usize)
            .map(|(node, _)| node)
            .collect()
    }
}

type NodeIndex = usize;

/// Represents a node in the NSW graph.
#[derive(Debug, Clone)]
struct Node {
    pub index: NodeIndex,
    pub vector: Vec<f32>,
    pub metadata: String,
    pub neighbors: Vec<NodeIndex>,
}

impl Node {
    pub fn new(
        index: NodeIndex,
        vector: Vec<f32>,
        metadata: String,
        neighbors: Vec<NodeIndex>,
    ) -> Self {
        Self {
            index,
            vector,
            metadata,
            neighbors,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut nsw = NSW::new(5555, 16);

    // Generate 20K random vectors
    let num_vectors = 20_000;

    for i in 0..num_vectors {
        let vector = generate_random_vector(1024);

        // if (i + 1) % 10000 == 0 {
        //     println!("Generated {} vectors", (i + 1).to_string().cyan());
        // }

        // Create a node with none neighbors for simplicity
        let node = Node::new(i, vector, "whatever".to_string(), vec![]);
        nsw.add_node_index_later(node);
    }

    // Load vector from sample embeddings

    // let embeddings = load_vector_from_sample().await;
    // let total_vectors = embeddings.total_vectors;
    //
    // // Insert/load all vectors into NSW
    // let mut chunks = embeddings.chunk.into_iter();
    // for (i, vector) in embeddings.embedding.into_iter().enumerate() {
    //     let metadata = chunks
    //         .next()
    //         .unwrap_or_else(|| format!("No metadata for index {}", i));
    //
    //     let node = Node::new(i, vector, metadata, vec![]);
    //     nsw.add_node_index_later(node);
    // }

    // Rearrange nodes to build the graph with neighbors
    println!(
        "Building NSW graph with {} nodes...",
        nsw.nodes.len().to_string().cyan()
    );
    let start_time = std::time::Instant::now();
    let graph = nsw.index_nodes();
    let duration = start_time.elapsed().as_secs_f64();
    println!("Rearranged in {}s", duration.to_string().yellow());

    // // Perform a query
    // let provider = Provider::init(
    //     "http://localhost:1234/v1/embeddings",
    //     "text-embedding-qwen3-embedding-0.6b",
    //     "local",
    // );
    // let sample_query = "What is this about?";
    // let query_embedding = provider.fetch_embedding(sample_query).await?;

    // let query_vector = query_embedding.embedding[0].clone();
    let query_vector = generate_random_vector(1024);
    // println!("\nQuery: {}", sample_query.to_string().yellow());
    println!("Querying vector: {:?}...", &query_vector[..3]);
    let top_k = 5;

    // Greedy Search
    // let start_time = std::time::Instant::now();
    // let results = greedy_search(&query_vector, top_k, &graph);
    // let duration = start_time.elapsed().as_secs_f64();
    // println!(
    //     "\nGreedy search completed in {}s",
    //     duration.to_string().yellow()
    // );
    // println!("\nTop {} Greedy Search Results:", top_k);
    // for (i, result) in results.iter().enumerate() {
    //     println!(
    //         "Result {}: Node Index: {}, Similarity: {:.4}",
    //         i + 1,
    //         result.node.index.to_string().cyan(),
    //         result.similarity.to_string().cyan()
    //     );
    // }

    // Parallel Greedy Search
    let greedy_start_time = std::time::Instant::now();
    let start_points = 5;
    let parallel_results = NSW::parallel_greedy_search(&query_vector, top_k, start_points, &graph);
    let duration = greedy_start_time.elapsed().as_secs_f64();
    println!(
        "\nParallel Greedy search with {} start points, completed in {}s",
        start_points.to_string().yellow(),
        duration.to_string().yellow()
    );
    println!("\nTop {} Parallel Greedy Search Results:", top_k);
    for (i, result) in parallel_results.iter().enumerate() {
        let similarity = cosine_similarity(&query_vector, &result.vector);
        println!(
            "Result {}: Node Index: {}, Similarity: {:.4}\n Metadata: {}",
            i + 1,
            result.index.to_string().cyan(),
            similarity.to_string().cyan(),
            result.metadata.to_string().dimmed().green()
        );
    }

    // Brute-force Search
    let bruteforce_start_time = std::time::Instant::now();
    let brute_results = NSW::brute_search(&query_vector, top_k, &graph);
    let duration = bruteforce_start_time.elapsed().as_secs_f64();
    println!(
        "\nBrute Force search completed in {}s",
        duration.to_string().yellow()
    );
    println!("\nTop {} Brute-force Results:", top_k);
    for (i, result) in brute_results.iter().enumerate() {
        let similarity = cosine_similarity(&query_vector, &result.vector);
        println!(
            "Result {}: Node Index: {}, Similarity: {:.4}, Metadata: {}",
            i + 1,
            result.index.to_string().cyan(),
            similarity.to_string().cyan(),
            result.metadata.to_string().dimmed().green()
        );
    }

    Ok(())
}

#[allow(unused)]
async fn get_cpu_usage() -> f32 {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_cpu_usage();

    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_usage();

    let cpu_usage =
        sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32; // Average CPU usage

    cpu_usage
}

#[allow(unused)]
#[deprecated = "use parallel_greedy_search instead for better performance and accuracy"]
/// Perform a single greedy search on built NSW graph
fn greedy_search(vector: &Vec<f32>, top_k: i32, nodes: &Vec<Node>) -> Vec<Node> {
    // Get a random start node
    let mut rng = rand::rng();
    let start_index = rng.random_range(0..nodes.len());
    let mut start_node = &nodes[start_index];

    let mut result_buffer = Vec::with_capacity(nodes.len()); // Pre-allocate

    loop {
        // Calculate similarity with the start node
        let similarity = cosine_similarity(vector, &start_node.vector);
        result_buffer.push((start_node.clone(), similarity));

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
    result_buffer.into_iter().map(|(node, _)| node).collect()
}
