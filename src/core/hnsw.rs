use crate::core::cosine_similarity;
use bincode::{Decode, Encode};
use rayon::iter::IndexedParallelIterator;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
pub struct HNSW {
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
}

impl HNSW {
    /// Creates a new HNSW instance with specified parameters.
    pub fn new(
        max_neighbors: usize,
        ef_construction: usize,
        max_layers: usize,
        distribution_bias: f32,
    ) -> Self {
        HNSW {
            nodes: Vec::with_capacity(10000), // Preallocate for efficiency
            entry_point: None,
            max_layers,
            max_neighbors,
            ef_construction,
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

    /// Insert a new node into the HNSW graph
    /// This is the core HNSW algorithm:
    /// 1. If first node, just add it as entry point
    /// 2. Otherwise, search from top layer down to find nearest neighbors
    /// 3. Connect the new node to its neighbors at each layer
    pub fn insert(&mut self, vector: Vec<f32>, metadata: String, max_level: usize) -> NodeId {
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
            vector,
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
            let selected: Vec<NodeId> = candidates
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
        let mut current_sim = self.similarity(query, &self.nodes[current].vector);
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
                    let neighbor_sim = self.similarity(query, &self.nodes[neighbor_id].vector);

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

        let entry_sim = self.similarity(query, &self.nodes[entry].vector);
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
                        let sim = self.similarity(query, &self.nodes[neighbor_id].vector);

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
    fn prune_connections(&mut self, node_id: NodeId, layer: usize) {
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

        let node_vector = self.nodes[node_id].vector.clone();

        // Calculate similarities to all neighbors
        let mut neighbor_sims: Vec<(NodeId, f32)> = self.nodes[node_id].neighbors[layer]
            .par_iter()
            .map(|&n| (n, self.similarity(&node_vector, &self.nodes[n].vector)))
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
    fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        cosine_similarity(a, b)
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
        let mut results: Vec<(NodeId, f32)> = candidates
            .into_par_iter()
            .map(|id| (id, self.similarity(query, &self.nodes[id].vector)))
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
}

/// Unique identifier for a node in the HNSW graph.
pub type NodeId = usize;

#[allow(unused)]
#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
/// Represents a node in the HNSW graph.
pub struct Node {
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
