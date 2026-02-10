mod utils;

#[allow(unused)]
use crate::utils::{cosine_similarity, generate_random_vector, load_sample_hnsw_index};
use anyhow::Result;
use blaze_db::core::{Metrics, dot_product, euclidean_similarity};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use rayon::iter::IndexedParallelIterator;
#[allow(unused)]
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
#[allow(unused)]
use rayon::prelude::{IntoParallelIterator, ParallelExtend};
use std::collections::{HashMap, HashSet};
use std::io::BufWriter;
use std::io::Write;
#[allow(unused)]
use std::sync::Mutex;

/// # Hierarchical Navigable Small World (HNSW)
///
/// ## Properties:
/// - **Hierarchical**: Multiple layers with exponentially decreasing nodes per layer
/// - **Navigable Small World**: Efficiently navigable graph structure at each layer
/// - **Logarithmic search complexity**: O(log N) by searching from top to bottom layers
/// - **Proper layer assignment**: Uses exponential distribution -ln(uniform) * 1/ln(M)
///
/// ## Algorithm highlights:
/// **Insert**: Search from top layer down, connect at each layer bidirectionally
/// **Search**: Greedy descent through upper layers, beam search at bottom layer
/// **Pruning**: Keep only M closest neighbors per node per layer

#[allow(unused)]
struct HNSW {
    /// All nodes in the graph, not layer-wise
    pub nodes: Vec<Node>,
    /// First node at the top layer, used as entry point for searches
    /// Can be a random node in top layer, just use the first inserted node for now TODO: think?
    pub entry_point: Option<NodeId>,
    /// Total number of layers in the graph
    pub max_layers: usize,
    /// Degree of each node (max number of neighbors) per layer
    pub max_neighbors: usize,
    /// Beam width during construction (higher = better quality but slower)
    /// Must be >= max_neighbors
    /// More values explored during insertion means better chance of finding good neighbors
    pub ef_construction: usize,
    /// Controls the layer distribution of nodes (exponential distribution bias) CURRENTLY UNUSED
    pub distribution_bias: f32,
    /// Metrics type: COSINE, EUCLIDEAN, RAW_DOT_PRODUCT
    pub metrics_type: Option<Metrics>,
}

impl HNSW {
    /// Creates a new HNSW instance with specified parameters.
    pub fn new(
        max_neighbors: usize,
        ef_construction: usize,
        max_layers: usize,
        distribution_bias: f32,
        metrics_type: Option<Metrics>,
    ) -> Self {
        HNSW {
            nodes: Vec::with_capacity(10_000), // Preallocate for efficiency
            entry_point: None,
            max_layers,
            max_neighbors,
            ef_construction,
            distribution_bias, // Currently unused
            metrics_type,      // Currently unused, default to cosine
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

    /// Insert a new node into the HNSW graph
    /// This is the core HNSW algorithm:
    /// 1. If first node, just add it as entry point
    /// 2. Otherwise, search from top layer down to find nearest neighbors
    /// 3. Connect the new node to its neighbors at each layer
    pub fn insert(&mut self, vector: &[f32], metadata: String, max_level: usize) -> NodeId {
        let node_id = self.nodes.len(); // TODO: Maybe use a better ID system later

        // println!(
        //     "\n[INSERT] Inserting node {} at max_level {}",
        //     node_id, max_level
        // );
        // log_debug_message(&format!(
        //     "\n[INSERT] Inserting node {} at max_level {}",
        //     node_id, max_level
        // ))
        // .ok();

        // Create the node with empty neighbor lists
        let node = Node {
            id: node_id,
            metadata,
            vector: vector.to_vec(),
            neighbors: vec![
                Vec::with_capacity(self.max_neighbors * self.max_layers);
                max_level + 1
            ], // Preallocate neighbor, neighbors * max_possible_level = total neighbors
            max_level,
        };

        // If this is the first node, set it as entry point
        if self.entry_point.is_none() {
            self.nodes.push(node);
            self.entry_point = Some(node_id);
            // println!("[INSERT] First node, set as entry point");
            // log_debug_message("[INSERT] First node, set as entry point").ok();
            return node_id;
        }

        // Store the new node's vector for distance calculations
        let new_vector = node.vector.clone();
        self.nodes.push(node);

        // Start search from entry point
        let mut current_nearest = self.entry_point.expect("Ohh no...entry_point is None");
        let entry_level = self.nodes[current_nearest].max_level;

        // println!(
        //     "[INSERT] Starting from entry point {} at level {}",
        //     current_nearest, entry_level
        // );
        // log_debug_message(&format!(
        //     "[INSERT] Starting from entry point {} at level {}",
        //     current_nearest, entry_level
        // ))
        // .ok();

        // Greedily traverse from top layer down to new node's level + 1
        // Just find the closest node, don't connect yet
        for layer in (max_level + 1..=entry_level).rev() {
            // println!("[INSERT] Greedy search at layer {}", layer);
            // log_debug_message(&format!("[INSERT] Greedy search at layer {}", layer)).ok();
            current_nearest = self.search_layer_greedy(&new_vector, current_nearest, layer);
            // println!(
            //     "[INSERT] Found nearest node {} at layer {}",
            //     current_nearest, layer
            // );
            // log_debug_message(&format!(
            //     "[INSERT] Found nearest node {} at layer {}",
            //     current_nearest, layer
            // ))
            // .ok();
        }

        // From new node's max_level down to 0, find neighbors and connect
        for layer in (0..=max_level).rev() {
            //println!("\n[INSERT] Processing layer {} for node {}", layer, node_id);
            // log_debug_message(&format!(
            //     "\n[INSERT] Processing layer {} for node {}",
            //     layer, node_id
            // ))
            // .ok();

            // Find ef_construction nearest neighbors at this layer
            let candidates =
                self.search_layer_knn(&new_vector, current_nearest, self.ef_construction, layer);

            // println!(
            //     "[INSERT] Found {} candidates at layer {}",
            //     candidates.len(),
            //     layer
            // );
            // log_debug_message(&format!(
            //     "[INSERT] Found {} candidates at layer {}",
            //     candidates.len(),
            //     layer
            // ))
            // .ok();

            // But only connect to max_neighbors of them
            let selected: Vec<blaze_db::prelude::NodeId> = candidates
                .into_par_iter()
                .take(self.max_neighbors)
                .collect();

            // println!("[INSERT] Selected {} neighbors to connect", selected.len());
            // log_debug_message(&format!(
            //     "[INSERT] Selected {} neighbors to connect",
            //     selected.len()
            // ))
            // .ok();

            // Connect new node to its neighbors (bidirectional)
            for &neighbor_id in &selected {
                // Add neighbor to new node's list
                self.nodes[node_id].neighbors[layer].push(neighbor_id);

                // Add new node to neighbor's list
                if layer <= self.nodes[neighbor_id].max_level {
                    self.nodes[neighbor_id].neighbors[layer].push(node_id);

                    // Prune neighbor's connections if it has too many
                    if self.nodes[neighbor_id].neighbors[layer].len() > self.max_neighbors {
                        // println!(
                        //     "[INSERT] Pruning neighbor {} (has {} connections)",
                        //     neighbor_id,
                        //     self.nodes[neighbor_id].neighbors[layer].len()
                        // );
                        // log_debug_message(&format!(
                        //     "[INSERT] Pruning neighbor {} (has {} connections)",
                        //     neighbor_id,
                        //     self.nodes[neighbor_id].neighbors[layer].len()
                        // ))
                        // .ok();
                        self.prune_connections(neighbor_id, layer);
                    }
                }
            }

            // Update current nearest for next layer
            if !selected.is_empty() {
                current_nearest = selected[0];
            }
        }

        // Update entry point if new node has higher level
        if max_level > entry_level {
            // println!(
            //     "[INSERT] New entry point: node {} at level {} (previous: {} at level {})",
            //     node_id, max_level, current_nearest, entry_level
            // );
            // log_debug_message(&format!(
            //     "[INSERT] New entry point: node {} at level {} (previous: {} at level {})",
            //     node_id, max_level, current_nearest, entry_level
            // ))
            // .ok();
            self.entry_point = Some(node_id);
        }

        node_id
    }

    /// Greedy search: find single closest node at a layer
    /// Used for navigating upper layers quickly
    fn search_layer_greedy(&self, query: &[f32], entry: NodeId, layer: usize) -> NodeId {
        let mut current = entry;
        let mut current_sim = self.similarity(
            query,
            &self.nodes[current].vector,
            self.metrics_type.as_ref(),
        );
        let mut improved = true;

        // println!(
        //     "[GREEDY] Starting at node {} (sim: {:.4}) on layer {}",
        //     entry, current_sim, layer
        // );
        // log_debug_message(&format!(
        //     "[GREEDY] Starting at node {} (sim: {:.4}) on layer {}",
        //     entry, current_sim, layer
        // ))
        // .ok();

        while improved {
            improved = false;

            // Check all neighbors at this layer
            if layer <= self.nodes[current].max_level {
                // println!(
                //     "[GREEDY] Exploring {} neighbors of node {} at layer {}",
                //     self.nodes[current].neighbors[layer].len(),
                //     current,
                //     layer
                // );
                // log_debug_message(&format!(
                //     "[GREEDY] Exploring {} neighbors of node {} at layer {}",
                //     self.nodes[current].neighbors[layer].len(),
                //     current,
                //     layer
                // ))
                // .ok();

                for &neighbor_id in &self.nodes[current].neighbors[layer] {
                    let neighbor_sim = self.similarity(
                        query,
                        &self.nodes[neighbor_id].vector,
                        self.metrics_type.as_ref(),
                    );

                    if neighbor_sim > current_sim {
                        current = neighbor_id;
                        current_sim = neighbor_sim;
                        improved = true;
                        // println!(
                        //     "[GREEDY] Improved to node {} with sim {:.4}",
                        //     current, current_sim
                        // );
                        // log_debug_message(&format!(
                        //     "[GREEDY] Improved to node {} with sim {:.4}",
                        //     current, current_sim
                        // ))
                        // .ok();
                    }
                }
            }
        }

        // println!(
        //     "[GREEDY] Final node: {} with sim {:.4}",
        //     current, current_sim
        // );
        // log_debug_message(&format!(
        //     "[GREEDY] Final node: {} with sim {:.4}",
        //     current, current_sim
        // ))
        // .ok();
        current
    }

    /// K-NN search at a specific layer: find K nearest neighbors
    /// Uses a BOUNDED beam search with ef parameter (critical for performance)
    fn search_layer_knn(
        &self,
        query: &[f32],
        entry: NodeId,
        ef: usize,
        layer: usize,
    ) -> Vec<NodeId> {
        // println!(
        //     "\n[KNN] Starting K-NN search at layer {} with ef={}",
        //     layer, ef
        // );
        // log_debug_message(&format!(
        //     "\n[KNN] Starting K-NN search at layer {} with ef={}",
        //     layer, ef
        // ))
        // .ok();

        let mut visited = HashSet::with_capacity(self.nodes.len());
        // Working set (to explore)
        let mut candidates = Vec::with_capacity(10000); // Preallocate
        // Best found so far
        let mut results = Vec::with_capacity(candidates.capacity());

        let entry_sim =
            self.similarity(query, &self.nodes[entry].vector, self.metrics_type.as_ref());
        visited.insert(entry);
        candidates.push((entry, entry_sim));
        results.push((entry, entry_sim));

        // println!("[KNN] Entry node {} has similarity {:.4}", entry, entry_sim);
        // log_debug_message(&format!(
        //     "[KNN] Entry node {} has similarity {:.4}",
        //     entry, entry_sim
        // ))
        // .ok();

        // Bounded beam search - only explore ef best candidates
        while let Some((current_id, current_sim)) = candidates.pop() {
            // println!(
            //     "[KNN] Exploring node {} with sim {:.4} (candidates: {}, results: {})",
            //     current_id,
            //     current_sim,
            //     candidates.len(),
            //     results.len()
            // );
            // log_debug_message(&format!(
            //     "[KNN] Exploring node {} with sim {:.4} (candidates: {}, results: {})",
            //     current_id,
            //     current_sim,
            //     candidates.len(),
            //     results.len()
            // ))
            // .ok();

            // Pruning: if current is worse than worst result, skip
            if !results.is_empty() {
                results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Higher similarity first
                if results.len() >= ef && current_sim < results[ef - 1].1 {
                    // println!(
                    //     "[KNN] Pruning candidate node {} (sim {:.4} < worst result {:.4})",
                    //     current_id,
                    //     current_sim,
                    //     results[ef - 1].1
                    // );
                    // log_debug_message(&format!(
                    //     "[KNN] Pruning candidate node {} (sim {:.4} < worst result {:.4})",
                    //     current_id,
                    //     current_sim,
                    //     results[ef - 1].1
                    // ))
                    // .ok();
                    continue; // Skip this candidate
                }
            }

            // Explore neighbors
            if layer <= self.nodes[current_id].max_level {
                //let neighbor_count = self.nodes[current_id].neighbors[layer].len();
                // println!(
                //     "[KNN] Node {} has {} neighbors at layer {}",
                //     current_id, neighbor_count, layer
                // );
                // log_debug_message(&format!(
                //     "[KNN] Node {} has {} neighbors at layer {}",
                //     current_id, neighbor_count, layer
                // ))
                // .ok();

                for &neighbor_id in &self.nodes[current_id].neighbors[layer] {
                    if visited.insert(neighbor_id) {
                        let sim = self.similarity(
                            query,
                            &self.nodes[neighbor_id].vector,
                            self.metrics_type.as_ref(),
                        );

                        // Only add if better than worst result or we haven't found ef results yet
                        if results.len() < ef || sim > results[ef - 1].1 {
                            // println!("[KNN] Adding neighbor {} with sim {:.4}", neighbor_id, sim);
                            // log_debug_message(&format!(
                            //     "[KNN] Adding neighbor {} with sim {:.4}",
                            //     neighbor_id, sim
                            // ))
                            // .ok();

                            candidates.push((neighbor_id, sim));
                            results.push((neighbor_id, sim));

                            // Keep results sorted and bounded
                            if results.len() > ef {
                                // println!(
                                //     "[KNN] Truncating results from {} to ef={}",
                                //     results.len(),
                                //     ef
                                // );
                                // log_debug_message(&format!(
                                //     "[KNN] Truncating results from {} to ef={}",
                                //     results.len(),
                                //     ef
                                // ))
                                // .ok();
                                results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                                results.truncate(ef);
                            }
                        }
                    }
                }
            }

            // Sort candidates by similarity (highest first) for next iteration
            candidates.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap()); // Lowest for pop()
        }

        // Return just the node IDs, already sorted by similarity (highest first)
        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        // println!("[KNN] Search complete, returning {} results", results.len());
        // log_debug_message(&format!(
        //     "[KNN] Search complete, returning {} results",
        //     results.len()
        // ))
        // .ok();

        results.into_par_iter().map(|(id, _)| id).collect()
    }

    /// Remove connections to keep only the M closest neighbors
    fn prune_connections(&mut self, node_id: blaze_db::prelude::NodeId, layer: usize) {
        // println!(
        //     "\n[PRUNE] Pruning node {} at layer {} (currently has {} neighbors)",
        //     node_id,
        //     layer,
        //     self.nodes[node_id].neighbors[layer].len()
        // );
        // log_debug_message(&format!(
        //     "\n[PRUNE] Pruning node {} at layer {} (currently has {} neighbors)",
        //     node_id,
        //     layer,
        //     self.nodes[node_id].neighbors[layer].len()
        // ))
        // .ok();

        // Calculate similarities to all neighbors
        let mut neighbor_sims: Vec<(blaze_db::prelude::NodeId, f32)> = self.nodes[node_id]
            .neighbors[layer]
            .par_iter()
            .map(|&n| {
                let sim = self.similarity(
                    &self.nodes[node_id].vector,
                    &self.nodes[n].vector,
                    self.metrics_type.as_ref(),
                );
                (n, sim)
            })
            .collect();

        // Keep only the M most similar
        neighbor_sims.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        neighbor_sims.truncate(self.max_neighbors);

        // println!(
        //     "[PRUNE] Keeping top {} neighbors for node {}",
        //     self.max_neighbors, node_id
        // );
        // log_debug_message(&format!(
        //     "[PRUNE] Keeping top {} neighbors for node {}",
        //     self.max_neighbors, node_id
        // ))
        // .ok();

        self.nodes[node_id].neighbors[layer] =
            neighbor_sims.into_par_iter().map(|(id, _)| id).collect();

        // println!(
        //     "[PRUNE] Node {} now has {} neighbors at layer {}",
        //     node_id,
        //     self.nodes[node_id].neighbors[layer].len(),
        //     layer
        // );
        // log_debug_message(&format!(
        //     "[PRUNE] Node {} now has {} neighbors at layer {}",
        //     node_id,
        //     self.nodes[node_id].neighbors[layer].len(),
        //     layer
        // ))
        // .ok();
    }

    /// Similarity metric: cosine similarity (higher = more similar)
    /// Returns a value in [-1, 1]:
    ///   - 1.0 = identical vectors (same direction)
    ///   - 0.0 = orthogonal vectors (perpendicular)
    ///   - -1.0 = opposite vectors
    ///
    /// For random high-dimensional vectors, similarities are typically around 0.0
    /// because random vectors are nearly orthogonal in high dimensions.
    #[inline]
    fn similarity(&self, a: &[f32], b: &[f32], metrics: Option<&Metrics>) -> f32 {
        match metrics {
            Some(Metrics::Cosine) | None => cosine_similarity(a, b),
            Some(Metrics::Euclidean) => euclidean_similarity(a, b),
            Some(Metrics::DotProduct) => dot_product(a, b),
        }
    }

    /// Public search API: find K nearest neighbors to a query
    /// Returns results as (NodeId, similarity) tuples sorted by similarity (highest first)
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)> {
        // println!("\n[SEARCH] Starting search for top {} neighbors", k);
        // log_debug_message(&format!(
        //     "\n[SEARCH] Starting search for top {} neighbors",
        //     k
        // ))
        // .ok();

        if self.entry_point.is_none() {
            // println!("[SEARCH] No entry point, returning empty results");
            // log_debug_message("[SEARCH] No entry point, returning empty results").ok();
            return Vec::new();
        }

        let entry = self.entry_point.unwrap();
        let entry_level = self.nodes[entry].max_level;
        let mut current = entry;

        // println!(
        //     "[SEARCH] Starting from entry point node {} at level {}",
        //     entry, entry_level
        // );
        // log_debug_message(&format!(
        //     "[SEARCH] Starting from entry point node {} at level {}",
        //     entry, entry_level
        // ))
        // .ok();

        // Traverse from top to layer 1
        for layer in (1..=entry_level).rev() {
            // println!("[SEARCH] Greedy search at layer {}", layer);
            // log_debug_message(&format!("[SEARCH] Greedy search at layer {}", layer)).ok();

            current = self.search_layer_greedy(query, current, layer);

            // println!("[SEARCH] Found node {} at layer {}", current, layer);
            // log_debug_message(&format!(
            //     "[SEARCH] Found node {} at layer {}",
            //     current, layer
            // ))
            // .ok();
        }

        // Search layer 0 thoroughly for K neighbors
        // println!(
        //     "[SEARCH] Performing K-NN search at layer 0 with ef={}",
        //     k * 2
        // );
        // log_debug_message(&format!(
        //     "[SEARCH] Performing K-NN search at layer 0 with ef={}",
        //     k * 2
        // ))
        // .ok();

        let candidates = self.search_layer_knn(query, current, k * 2, 0);

        // println!("[SEARCH] Found {} candidates at layer 0", candidates.len());
        // log_debug_message(&format!(
        //     "[SEARCH] Found {} candidates at layer 0",
        //     candidates.len()
        // ))
        // .ok();

        // Return with similarities
        let mut results: Vec<(blaze_db::prelude::NodeId, f32)> = candidates
            .into_par_iter()
            .map(|id| {
                (
                    id,
                    self.similarity(query, &self.nodes[id].vector, self.metrics_type.as_ref()),
                )
            })
            .collect();

        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Higher similarity first
        results.truncate(k);

        // println!("[SEARCH] Returning top {} results", results.len());
        // log_debug_message(&format!("[SEARCH] Returning top {} results", results.len())).ok();

        results
    }

    /// Search and return results with metadata
    /// Returns results as (NodeId, similarity, metadata) tuples sorted by similarity (highest first)
    pub fn search_with_metadata(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32, &str)> {
        let results = self.search(query, k);
        results
            .into_iter() // TODO:  parallel iterator not needed here?
            .map(|(id, sim)| (id, sim, self.nodes[id].metadata.as_str()))
            .collect()
    }

    /// Get metadata for a specific node
    pub fn get_metadata(&self, node_id: NodeId) -> Option<&String> {
        self.nodes.get(node_id).map(|node| &node.metadata)
    }

    pub fn brute_force_search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)> {
        let mut results: Vec<(NodeId, f32)> = self
            .nodes
            .par_iter()
            .enumerate()
            .map(|(id, node)| {
                (
                    id,
                    self.similarity(query, &node.vector, self.metrics_type.as_ref()),
                )
            })
            .collect();

        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Higher similarity first
        results.truncate(k);

        results
    }

    pub fn brute_force_search_with_metadata(
        &self,
        query: &[f32],
        k: usize,
    ) -> Vec<(NodeId, f32, &str)> {
        let results = self.brute_force_search(query, k);
        results
            .into_iter() // TODO:  parallel iterator not needed here?
            .map(|(id, sim)| (id, sim, self.nodes[id].metadata.as_str()))
            .collect()
    }
}

/// Unique identifier for a node in the HNSW graph.
type NodeId = usize;

#[allow(unused)]
#[derive(Debug)]
/// Represents a node in the HNSW graph.
struct Node {
    /// Unique identifier for the node
    pub id: NodeId,
    /// Metadata associated with the node
    pub metadata: String, // String for now, I guess? TODO: make generic?
    /// Vector representation of the node, any dimensionality
    pub vector: Vec<f32>,
    /// Neighbors per layer, e.g neighbors[0] is the list of neighbors in layer 0
    pub neighbors: Vec<Vec<NodeId>>,
    /// The highest layer this node exists in
    pub max_level: usize,
}

impl Node {
    /// Creates a new Node with the given id, vector, metadata, and max_level.
    #[allow(unused)]
    pub fn new(id: NodeId, vector: Vec<f32>, metadata: String, max_level: usize) -> Self {
        Node {
            id,
            metadata,
            vector,
            neighbors: vec![Vec::new(); max_level + 1], // Preallocate neighbor lists
            max_level,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut hnsw = HNSW::new(16, 200, 12, 0.8, Some(Metrics::Cosine));

    // create_debug_log_file().await?;

    // get_level_math_debug(&hnsw).await?;

    // let loaded_vector_data = load_sample_hnsw_index().await;

    let node_count = 10_000 * 2;
    // let node_count = loaded_vector_data.hnsw_store.nodes.len();
    let dimension = 1024;

    println!(
        "\nBuilding HNSW graph with {} nodes...",
        node_count.to_string().cyan()
    );

    // Progress bar setup
    let progress_bar = ProgressBar::new(node_count as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")?
            .progress_chars("●●•-"),
    );

    let load_time = std::time::Instant::now();
    for _i in 0..node_count {
        let vector = generate_random_vector(dimension);
        let level = hnsw.get_random_level();
        let metadata = "what a nice vector".to_string();
        progress_bar.inc(1);
        hnsw.insert(&*vector, metadata, level);
    }
    progress_bar.finish_and_clear();

    println!("Indexing completed in {:?}", load_time.elapsed());

    // Print layer statistics
    print_layer_stats(&hnsw);

    // Perform a query
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
    println!("Querying vector: {:?}...", &query_vector.as_slice()[..3]);
    let top_k = 5;

    let start = std::time::Instant::now();
    let results = hnsw.search_with_metadata(&query_vector, top_k);
    let search_time = start.elapsed().as_secs_f64();
    println!("Search completed in: {}s", search_time.to_string().yellow());

    println!("\nTop {} nearest neighbors:", top_k);

    for (i, (node_id, similarity, _metadata)) in results.iter().enumerate() {
        println!(
            "  {}. Node {:5} - similarity: {:.4}, Metadata: {}",
            i + 1,
            node_id.to_string().yellow(),
            similarity.to_string().green(),
            HNSW::get_metadata(&hnsw, *node_id)
                .unwrap()
                .to_string()
                .dimmed()
                .green()
        );
    }

    let start_brute = std::time::Instant::now();
    let brute_results = hnsw.brute_force_search_with_metadata(&query_vector, top_k);
    let brute_time = start_brute.elapsed().as_secs_f64();
    println!(
        "\nBrute-force search completed in: {}s",
        brute_time.to_string().yellow()
    );

    println!("\nTop {} nearest neighbors (Brute-force):", top_k);

    for (i, (node_id, similarity, _metadata)) in brute_results.iter().enumerate() {
        println!(
            "  {}. Node {:5} - similarity: {:.4}, Metadata: {}",
            i + 1,
            node_id.to_string().yellow(),
            similarity.to_string().green(),
            HNSW::get_metadata(&hnsw, *node_id)
                .unwrap()
                .to_string()
                .dimmed()
                .green()
        );
    }

    let speedup = brute_time / search_time;
    println!("\nSpeedup over brute-force: {:.2}x", speedup);

    Ok(())
}

#[allow(unused)]
fn print_layer_stats(hnsw: &HNSW) {
    let mut layer_counts = vec![0usize; hnsw.max_layers];

    for node in &hnsw.nodes {
        for layer in 0..=node.max_level {
            if layer < layer_counts.len() {
                layer_counts[layer] += 1;
            }
        }
    }

    println!("\nHNSW Layer Statistics:");
    for (layer, count) in layer_counts.iter().enumerate() {
        if *count > 0 {
            let percentage = (*count as f32 / hnsw.nodes.len() as f32) * 100.0;
            println!("  Layer {}: {} nodes ({:.2}%)", layer, count, percentage);
        }
    }
    if let Some(entry) = hnsw.entry_point {
        println!(
            "  Entry point: node {} at layer {}",
            entry, hnsw.nodes[entry].max_level
        );
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

/// Returns 6+6 hardcoded 3d and a 3d random generated vectors for testing and debugging
#[allow(unused)]
async fn get_test_vectors() -> (Vec<Vec<f32>>, Vec<f32>) {
    // Generate a random vector
    let mut rng = rand::rng();

    let random_vector = vec![
        rng.random_range(-2.0..2.0),
        rng.random_range(-2.0..2.0),
        rng.random_range(-2.0..2.0),
    ];

    let hardcoded = vec![
        vec![1.0, 0.0, 0.0],
        vec![1.41, 1.41, 0.0],
        vec![0.0, 1.0, 0.0],
        vec![0.0, 1.41, 1.41],
        vec![0.0, 0.0, 1.0],
        vec![1.41, 0.0, 1.41],
        vec![-1.0, 0.0, 0.0],
        vec![-1.41, -1.41, 0.0],
        vec![0.0, -1.0, 0.0],
        vec![0.0, -1.41, -1.41],
        vec![0.0, 0.0, -1.0],
        vec![-1.41, 0.0, -1.41],
    ];

    (hardcoded, random_vector)
}

/// Creates a debug log file for capturing the debug statements output.
#[allow(unused)]
async fn create_debug_log_file() -> Result<()> {
    // Create or truncate the log file
    tokio::fs::File::create("hnsw_debug.log").await?;

    // Write header
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let header = format!(
        "HNSW Debug Log \nStarted at: {}\n{}\n",
        timestamp,
        "=".repeat(50)
    );

    tokio::fs::write("hnsw_debug.log", header).await?;
    println!("Created debug log file: hnsw_debug.log");
    Ok(())
}

/// Appends a debug message to the log file with timestamp.
#[inline]
#[allow(unused)]
fn log_debug_message(message: &str) -> Result<()> {
    use std::fs::OpenOptions;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("hnsw_debug.log");

    if let Ok(f) = file {
        let mut writer = BufWriter::new(f);
        // Add a simple counter-based timestamp for performance
        if let Err(e) = writeln!(writer, "{}", message) {
            eprintln!("Warning: Failed to write to log file: {}", e);
        }
        // Flush to ensure data is written
        let _ = writer.flush();
    } else {
        eprintln!("Warning: Failed to open log file");
    }

    Ok(())
}
