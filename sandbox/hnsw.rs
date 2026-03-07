mod utils;

#[allow(unused)]
use crate::utils::{generate_random_vectors, load_index_from_example};
use anyhow::Result;
use blaze_db::core::{Metrics, cosine_similarity, dot_product, euclidean_similarity};
use blaze_db::prelude::VectorData;
use blaze_db::utils::Provider;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rand::RngExt;
use rayon::iter::IndexedParallelIterator;
#[allow(unused)]
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
#[allow(unused)]
use rayon::prelude::{IntoParallelIterator, ParallelExtend};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
#[allow(unused)]
use std::sync::Mutex;
use uuid::Uuid;

/// Priority queue entry for nodes to explore during search.
#[derive(Clone, Copy)]
struct Candidate(NodeIndex, f32);

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl Eq for Candidate {}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // pop() gives us the HIGHEST similarity candidate
        self.1.partial_cmp(&other.1).unwrap()
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// We want to quickly find the WORST result in our top-k (to know when to prune)
/// By reversing the comparison, the heap top is the LOWEST similarity in our results
/// pop() gives us the worst result, making pruning O(1)
#[derive(Clone, Copy)]
struct ScoredResult(NodeIndex, f32);

impl PartialEq for ScoredResult {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl Eq for ScoredResult {}

impl Ord for ScoredResult {
    fn cmp(&self, other: &Self) -> Ordering {
        // peek() gives us the WORST result in our top-k
        other.1.partial_cmp(&self.1).unwrap()
    }
}

impl PartialOrd for ScoredResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const DEFAULT_EF_MULTIPLIER: usize = 4;

const DEFAULT_EF_INC_FACTOR: f32 = 1.5;

/// # Hierarchical Navigable Small World (Hnsw)
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
/// **Tombstones**: Mark deleted nodes and skip during search, periodic cleanup
pub struct Hnsw {
    /// All nodes in the graph, not layer-wise
    pub nodes: Vec<Node>,
    /// First node at the top layer, used as entry point for searches
    pub entry_point: Option<NodeIndex>,
    /// Total number of layers in the graph
    pub max_layers: usize,
    /// Degree of each node (max number of neighbors) per layer
    pub max_neighbors: usize,
    /// More values explored during insertion means better chance of finding good neighbors
    pub ef_construction: usize,
    /// Controls the layer distribution of nodes (exponential distribution bias) CURRENTLY UNUSED
    pub distribution_bias: f32,
    /// Similarity metric to use for distance calculations (default: Cosine)
    pub metrics: Option<Metrics>,
    /// Mapping from external ID (set by params) to internal ID (array index)
    /// It's just a fucking HashMap for O(1) lookups, that's all it is, nothing fancy
    id_mapper: HashMap<NodeID, NodeIndex>,
}

impl Default for Hnsw {
    fn default() -> Self {
        Hnsw::new(16, 200, 16, 1.0, &Some(Metrics::Cosine))
    }
}

impl Hnsw {
    /// Creates a new Hnsw instance with specified parameters.
    pub fn new(
        max_neighbors: usize,
        ef_construction: usize,
        max_layers: usize,
        distribution_bias: f32,
        metrics: &Option<Metrics>,
    ) -> Self {
        Hnsw {
            nodes: Vec::with_capacity(128_000),
            entry_point: None,
            max_layers,
            max_neighbors,
            ef_construction,
            distribution_bias,        // Currently unused
            metrics: metrics.clone(), // Currently unused, default to cosine
            id_mapper: HashMap::with_capacity(128_000),
        }
    }

    /// Generates a random level for a new node based on an exponential distribution.
    /// Uses the Hnsw paper formula: floor(-ln(rand) * 1/ln(M))
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

    /// Insert a new node into the Hnsw graph
    /// This is the core Hnsw algorithm:
    /// 1. If first node, just add it as entry point
    /// 2. Otherwise, search from top layer down to find nearest neighbors
    /// 3. Connect the new node to its neighbors at each layer
    ///
    /// # Arguments
    /// * `node_id` - User-provided external ID (must be unique)
    /// * `vector` - The vector to insert
    /// * `metadata` - Metadata associated with the node
    /// * `max_level` - The maximum level for this node
    ///
    /// # Returns
    /// * `Ok(NodeId)` - The array index of the newly inserted node in the `nodes` vector
    /// * `Err` - If the node_id already exists
    pub fn insert(
        &mut self,
        vector_id: String,
        vector: &[f32],
        metadata: String,
        max_level: usize,
    ) -> Result<NodeIndex> {
        // Check for duplicate node_id
        if self.id_mapper.contains_key(&vector_id) {
            return Err(anyhow::anyhow!(
                "External ID '{}' already exists",
                vector_id
            ));
        }

        let node_id = self.nodes.len();

        // Create the node with empty neighbor lists
        let node = Node {
            node_id: vector_id.clone(),
            metadata,
            vector: vector.to_vec(),
            neighbors: vec![
                Vec::with_capacity(self.max_neighbors * self.max_layers);
                max_level + 1
            ],
            max_level,
            tombstone: false,
        };

        // Register in ID mapper for O(1) lookups
        self.id_mapper.insert(vector_id, node_id);

        if self.entry_point.is_none() {
            self.nodes.push(node);
            self.entry_point = Some(node_id);
            // println!("[INSERT] First node, set as entry point");
            // log_debug_message("[INSERT] First node, set as entry point").ok();
            return Ok(node_id);
        }

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
            let selected: Vec<NodeIndex> = candidates
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
                // Only connect if neighbor exists at this layer
                if layer <= self.nodes[neighbor_id].max_level {
                    // Add neighbor to new node's list
                    self.nodes[node_id].neighbors[layer].push(neighbor_id);

                    // Add new node to neighbor's list
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

        Ok(node_id)
    }

    /// Greedy search: find single closest node at a layer
    /// Used for navigating upper layers quickly
    fn search_layer_greedy(&self, query: &[f32], entry: NodeIndex, layer: usize) -> NodeIndex {
        let mut current = entry;
        let mut current_sim =
            self.similarity(query, &self.nodes[current].vector, self.metrics.as_ref());
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
                    // Skip tombstoned nodes during search
                    if self.nodes[neighbor_id].tombstone {
                        continue;
                    }
                    let neighbor_sim = self.similarity(
                        query,
                        &self.nodes[neighbor_id].vector,
                        self.metrics.as_ref(),
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
    /// Uses a BOUNDED search with ef parameter (critical for performance)
    ///
    /// ALGORITHM:
    /// 1. Start with entry point in both candidates and results
    /// 2. Pop highest similarity candidate from heap (best-first)
    /// 3. If candidate is worse than our worst result, skip (prune)
    /// 4. Otherwise, explore all its neighbors
    /// 5. Add promising neighbors to candidates AND results (if better than worst)
    /// 6. Repeat until candidates empty
    /// 7. Return top-k results sorted by similarity
    ///
    /// COMPLEXITY: O(log n) per operation instead of O(n log n)
    fn search_layer_knn(
        &self,
        query: &[f32],
        entry: NodeIndex,
        ef: usize,
        layer: usize,
    ) -> Vec<NodeIndex> {
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

        // CANDIDATES heap: explore highest similarity first
        // pop() gives us the most promising node to explore next
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::with_capacity(ef * 2);

        // RESULTS heap: track top-k results
        // peek() gives us the WORST result in our top-k (for pruning)
        let mut results: BinaryHeap<ScoredResult> = BinaryHeap::with_capacity(ef);

        let entry_sim = self.similarity(query, &self.nodes[entry].vector, self.metrics.as_ref());
        visited.insert(entry);
        candidates.push(Candidate(entry, entry_sim));
        results.push(ScoredResult(entry, entry_sim));

        // println!("[KNN] Entry node {} has similarity {:.4}", entry, entry_sim);
        // log_debug_message(&format!(
        //     "[KNN] Entry node {} has similarity {:.4}",
        //     entry, entry_sim
        // ))
        // .ok();

        while let Some(Candidate(current_id, current_sim)) = candidates.pop() {
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
            // PRUNING: if we've filled ef slots and current is worse than our worst result,
            // there's no point exploring it - all its neighbors will be even worse
            if let Some(worst_result) = results.peek()
                && results.len() >= ef
                && current_sim < worst_result.1
            {
                continue;
            }

            // Explore neighbors of current candidate
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
                    // Skip tombstoned nodes during search
                    if self.nodes[neighbor_id].tombstone {
                        continue;
                    }

                    if visited.insert(neighbor_id) {
                        let sim = self.similarity(
                            query,
                            &self.nodes[neighbor_id].vector,
                            self.metrics.as_ref(),
                        );

                        // WHATDAFAK: should we add this neighbor to our search frontier?
                        // Add if: we haven't filled ef slots OR new node is better than our worst
                        let worst_if_full = results.peek().map(|r| r.1);
                        let should_add = match (results.len(), worst_if_full) {
                            (len, _) if len < ef => true,            // still filling
                            (_, Some(worst)) if sim > worst => true, // better than worst
                            _ => false,
                        };

                        if should_add {
                            candidates.push(Candidate(neighbor_id, sim));
                            results.push(ScoredResult(neighbor_id, sim));

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
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        // Final sort for highest similarity first for output consistency (results is a min-heap by similarity, so we reverse it)
        let mut sorted_results: Vec<ScoredResult> = results.into_vec();
        sorted_results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // println!("[KNN] Search complete, returning {} results", sorted_results.len());
        // log_debug_message(&format!(
        //     "[KNN] Search complete, returning {} results",
        //     results.len()
        // ))
        // .ok();

        sorted_results
            .into_iter()
            .map(|ScoredResult(id, _)| id)
            .collect()
    }

    /// Remove connections to keep only the M closest neighbors
    fn prune_connections(&mut self, node_id: NodeIndex, layer: usize) {
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

        // Store the old neighbor list to identify which edges to remove
        let old_neighbors: HashSet<NodeIndex> = self.nodes[node_id].neighbors[layer]
            .iter()
            .copied()
            .collect();

        // Calculate similarities to all neighbors, filtering out tombstoned nodes
        let mut neighbor_sims: Vec<(NodeIndex, f32)> = self.nodes[node_id].neighbors[layer]
            .par_iter()
            .filter(|&&n| !self.nodes[n].tombstone) // Skip tombstoned neighbors
            .map(|&n| {
                let sim = self.similarity(
                    &self.nodes[node_id].vector,
                    &self.nodes[n].vector,
                    self.metrics.as_ref(),
                );
                (n, sim)
            })
            .collect();

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
        let new_neighbors: Vec<NodeIndex> =
            neighbor_sims.into_par_iter().map(|(id, _)| id).collect();
        let new_neighbors_set: HashSet<NodeIndex> = new_neighbors.iter().copied().collect();

        let removed_neighbors: Vec<NodeIndex> = old_neighbors
            .difference(&new_neighbors_set)
            .copied()
            .collect();

        // Update this node's neighbor list
        self.nodes[node_id].neighbors[layer] = new_neighbors;

        // CRITICAL: Remove reverse edges from pruned neighbors to maintain bidirectionality
        for removed_neighbor_id in removed_neighbors {
            if layer <= self.nodes[removed_neighbor_id].max_level {
                self.nodes[removed_neighbor_id].neighbors[layer].retain(|&n| n != node_id);
            }
        }

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

    /// Similarity metric: cosine similarity, Euclidean similarity, or raw dot product
    #[inline]
    fn similarity(&self, a: &[f32], b: &[f32], metrics: Option<&Metrics>) -> f32 {
        match metrics {
            Some(Metrics::Cosine) | None => cosine_similarity(a, b),
            Some(Metrics::Euclidean) => euclidean_similarity(a, b),
            Some(Metrics::DotProduct) => dot_product(a, b),
        }
    }

    /// Public search API: find K nearest neighbors to a query
    /// Returns results as (node_id, similarity) tuples sorted by similarity (highest first)
    ///
    /// # Arguments
    /// * `query` - The query vector
    /// * `k` - Number of nearest neighbors to return
    /// * `ef_search` - Optional beam width for search. If None, uses k * DEFAULT_EF_MULTIPLIER
    pub fn search(&self, query: &[f32], k: usize, ef_search: Option<usize>) -> Vec<(String, f32)> {
        if self.entry_point.is_none() {
            return Vec::new();
        }

        let ef = ef_search.unwrap_or(k * DEFAULT_EF_MULTIPLIER);
        let mut current_ef = ef;
        // TODO: Need to cap this otherwise...
        let max_ef = self.nodes.len().max(ef);

        loop {
            let results = self.search_internal(query, k, current_ef);

            // Filter out tombstoned nodes and convert to external IDs
            let active_results: Vec<(String, f32)> = results
                .into_par_iter()
                .filter(|(id, _)| !self.nodes[*id].tombstone)
                .map(|(id, sim)| (self.nodes[id].node_id.clone(), sim))
                .collect();

            // If we have enough results, or we've reached max ef, return
            if active_results.len() >= k || current_ef >= max_ef {
                return active_results.into_par_iter().take(k).collect();
            }

            // Not enough active results, perform DOMAIN EXPANSION 🟣
            current_ef = (current_ef as f32 * DEFAULT_EF_INC_FACTOR) as usize;
            current_ef = current_ef.min(max_ef);
        }
    }

    /// Internal search method that performs the actual Hnsw search
    /// Returns array index and similarity of candidates, including tombstoned nodes
    #[inline]
    pub fn search_internal(&self, query: &[f32], k: usize, ef: usize) -> Vec<(NodeIndex, f32)> {
        let entry = self.entry_point.expect("Entry point should exist");
        let entry_level = self.nodes[entry].max_level;
        let mut current = entry;

        // Traverse from top to layer 1
        for layer in (1..=entry_level).rev() {
            current = self.search_layer_greedy(query, current, layer);
        }

        // Search layer 0 thoroughly for K neighbors!!!
        let candidates = self.search_layer_knn(query, current, ef, 0);

        // Return with similarities
        let mut results: Vec<(NodeIndex, f32)> = candidates
            .into_par_iter()
            .map(|id| {
                (
                    id,
                    self.similarity(query, &self.nodes[id].vector, self.metrics.as_ref()),
                )
            })
            .collect();

        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(k);

        results
    }

    #[inline]
    /// Search and return results with metadata
    /// Returns results as (node_id, similarity, metadata) tuples sorted by similarity (highest first)
    pub fn search_with_metadata(
        &self,
        query: &[f32],
        k: usize,
        ef_search: Option<usize>,
    ) -> Vec<(String, f32, String)> {
        let results = self.search(query, k, ef_search);
        results
            .into_iter()
            .map(|(node_id, sim)| {
                let metadata = self
                    .id_mapper
                    .get(&node_id)
                    .and_then(|&id| self.nodes.get(id))
                    .map(|node| node.metadata.clone())
                    .unwrap_or_default();
                (node_id, sim, metadata)
            })
            .collect()
    }

    #[inline]
    /// Brute-force search for testing and validation. Returns all nodes sorted by similarity (highest first).
    pub fn brute_force_search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut results: Vec<(String, f32)> = self
            .nodes
            .par_iter()
            .filter(|node| !node.tombstone) // Filter out tombstoned nodes
            .map(|node| {
                (
                    node.node_id.clone(),
                    self.similarity(query, &node.vector, self.metrics.as_ref()),
                )
            })
            .collect();

        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(k);

        results
    }

    #[inline]
    pub fn brute_force_search_with_metadata(
        &self,
        query: &[f32],
        k: usize,
    ) -> Vec<(String, f32, String)> {
        let results = self.brute_force_search(query, k);
        results
            .into_iter()
            .map(|(node_id, sim)| {
                let metadata = self
                    .get_node_by_id(&node_id)
                    .map(|node| node.metadata.clone())
                    .unwrap_or_default();
                (node_id, sim, metadata)
            })
            .collect()
    }

    /// Delete a node by external ID
    /// If the deleted node is the entry point, finds a new one
    #[inline]
    pub fn delete_node_by_id(&mut self, node_id: &str) -> Result<()> {
        let node_id = self
            .id_mapper
            .get(node_id)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("External ID '{}' not found", node_id))?;

        // Mark as tombstone
        self.mark_tombstone(node_id)?;

        // If this was the entry point, find a new one
        if let Some(entry) = self.entry_point
            && entry == node_id
        {
            self.set_new_entry_point();
        }

        Ok(())
    }

    /// Get node by ID
    #[inline]
    pub fn get_node_by_id(&self, node_id: &str) -> Option<&Node> {
        self.id_mapper
            .get(node_id)
            .and_then(|&id| self.nodes.get(id))
    }

    #[inline]
    /// Returns the count of active (non-tombstoned) nodes
    pub fn active_count(&self) -> usize {
        self.nodes.iter().filter(|node| !node.tombstone).count()
    }

    #[inline]
    /// Returns the count of tombstoned (deleted) nodes
    pub fn tombstone_count(&self) -> usize {
        self.nodes.iter().filter(|node| node.tombstone).count()
    }

    #[inline]
    /// Returns the ratio of tombstoned nodes to total nodes
    pub fn tombstone_ratio(&self) -> f32 {
        if self.nodes.is_empty() {
            0.0
        } else {
            self.tombstone_count() as f32 / self.nodes.len() as f32
        }
    }

    #[inline]
    /// Mark a node as a tombstone for lazy deletion. This allows us to keep the graph structure intact while ignoring "deleted" nodes during search or index.
    fn mark_tombstone(&mut self, node_id: NodeIndex) -> Result<NodeIndex> {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.tombstone = true;
            Ok(node_id)
        } else {
            Err(anyhow::anyhow!("Node ID {} does not exist", node_id))
        }
    }

    /// Find and sets new entry point when the current one is deleted
    /// Searches from max_layer down to find the highest-level active node
    #[inline]
    fn set_new_entry_point(&mut self) {
        for layer in (0..self.max_layers).rev() {
            for (id, node) in self.nodes.iter().enumerate() {
                if node.max_level == layer && !node.tombstone {
                    self.entry_point = Some(id);
                    return;
                }
            }
        }
        // No active nodes found
        self.entry_point = None;
    }

    /// Rebuilds the Hnsw index by removing all tombstoned nodes
    /// This creates a new compact index with only active nodes
    /// Note: Node IDs will be remapped (compacted)
    pub fn reindex(&mut self) -> Result<()> {
        if self.tombstone_count() == 0 {
            return Ok(()); // We are good, no need to reindex
        }

        let mut old_to_new: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut new_nodes: Vec<Node> = Vec::with_capacity(self.active_count());

        // Create new mapping
        let mut new_id_mapper: HashMap<String, NodeIndex> =
            HashMap::with_capacity(self.active_count());

        // First pass: copy active nodes and build ID mapping
        for (old_id, node) in self.nodes.iter().enumerate() {
            if !node.tombstone {
                let new_id = new_nodes.len();
                old_to_new.insert(old_id, new_id);

                new_id_mapper.insert(node.node_id.clone(), new_id);

                // Create new node without neighbors (rebuild them later)
                let new_node = Node {
                    node_id: node.node_id.clone(),
                    metadata: node.metadata.clone(),
                    vector: node.vector.clone(),
                    neighbors: vec![Vec::new(); node.max_level + 1],
                    max_level: node.max_level,
                    tombstone: false,
                };
                new_nodes.push(new_node);
            }
        }

        // Second pass: rebuild neighbor connections with new IDs
        for (old_id, node) in self.nodes.iter().enumerate() {
            if node.tombstone {
                continue;
            }

            let new_id = *old_to_new.get(&old_id).unwrap();

            for layer in 0..=node.max_level {
                for &old_neighbor_id in &node.neighbors[layer] {
                    // Skip if neighbor is tombstoned
                    if self.nodes[old_neighbor_id].tombstone {
                        continue;
                    }

                    // Map to new ID
                    if let Some(&new_neighbor_id) = old_to_new.get(&old_neighbor_id) {
                        new_nodes[new_id].neighbors[layer].push(new_neighbor_id);
                    }
                }
            }
        }

        // Update entry point
        if let Some(old_entry) = self.entry_point {
            if !self.nodes[old_entry].tombstone {
                self.entry_point = old_to_new.get(&old_entry).copied();
            } else {
                // Find new entry point (highest level active node)
                self.entry_point = None;
                for (new_id, node) in new_nodes.iter().enumerate() {
                    if self.entry_point.is_none()
                        || node.max_level > new_nodes[self.entry_point.unwrap()].max_level
                    {
                        self.entry_point = Some(new_id);
                    }
                }
            }
        }

        // Replace nodes with new compact version
        self.nodes = new_nodes;

        // Update external ID mapping
        self.id_mapper = new_id_mapper;

        Ok(())
    }
}

/// It's just a fucking array index, that ip_mapper in Hnsw? It's just for fucking O(1)
pub type NodeIndex = usize;

/// Unique identifier for a node. (Stable across reindexing)
pub type NodeID = String;

/// Represents a node in the Hnsw graph.
#[derive(Clone)]
pub struct Node {
    /// External identifier - stable across reindexing
    pub node_id: NodeID,
    /// Metadata associated with the node
    pub metadata: String, // String for now, I guess? TODO: make generic or JSON VALUE?
    /// Vector representation of the node, any dimensionality
    pub vector: Vec<f32>,
    /// Neighbors per layer, e.g neighbors[0] is the list of neighbors in layer 0
    pub neighbors: Vec<Vec<NodeIndex>>,
    /// The highest layer this node exists in
    pub max_level: usize,
    /// Flag 🪦 for lazy deletion
    tombstone: bool,
}

impl Node {
    /// Creates a new Node with the given id, vector, metadata, and max_level.
    pub fn new(id: String, vector: Vec<f32>, metadata: String, max_level: usize) -> Self {
        Node {
            node_id: id,
            metadata,
            vector,
            neighbors: vec![Vec::new(); max_level + 1], // Preallocate neighbor lists
            max_level,
            tombstone: false,
        }
    }

    /// Returns true if this node has been soft-deleted (tombstoned).
    #[inline]
    pub fn is_deleted(&self) -> bool {
        self.tombstone
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut hnsw = Hnsw::new(16, 200, 12, 0.8, &Some(Metrics::Cosine));

    let node_count = 10_000 * 3;
    // let dimension = 1024;

    // Compute the random vector here, not during insertion
    // let vectors = generate_random_vectors(node_count, dimension);

    println!(
        "\nBuilding Hnsw graph with {} nodes...",
        node_count.to_string().cyan()
    );

    // Progress bar setup
    let progress_bar = ProgressBar::new(node_count as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:60.cyan/blue}] {pos}/{len} ({percent}%)")?
            .progress_chars("●●•-"),
    );
    let provider = Provider::init(
        "http://localhost:1234/v1/embeddings",
        "text-embedding-qwen3-embedding-0.6b",
        "local",
    );

    let vector_data =
        VectorData::read_from_disk(&PathBuf::from("./embeddings/Amazon_EMBEDDINGS.bin")).await?;

    let nodes_to_index = vector_data.size().min(node_count) as u64;

    // let load_time = std::time::Instant::now();
    // for (_i, vector) in vectors.into_iter().enumerate() {
    //     let vector = vector;
    //     let level = hnsw.get_random_level();
    //     let metadata = "what a nice vector".to_string();
    //     let uuid = Uuid::new_v4().to_string();
    //     let external_id = format!("node_{}", uuid);
    //     progress_bar.inc(1);
    //
    //     hnsw.insert(external_id, &*vector, metadata, level)?;
    // }

    let start_indexing = std::time::Instant::now();
    for i in 0..nodes_to_index as usize {
        let embedding = match vector_data.get_vector(i) {
            Some(v) => v,
            None => break,
        };
        let metadata = match vector_data.get_chunk(i) {
            Some(m) => m,
            None => break,
        };
        let random_level = hnsw.get_random_level();
        let uuid = Uuid::new_v4().to_string();
        let node_id = format!("node_{}", uuid);
        hnsw.insert(node_id, embedding, metadata.to_string(), random_level)?;
        progress_bar.inc(1);
    }

    progress_bar.finish_and_clear();

    println!("Indexing completed in {:?}", start_indexing.elapsed());

    verify_bidirectional_edges(&hnsw)?;

    // Print layer statistics
    print_layer_stats(&hnsw);

    let query_sample = "Headphones";
    let query_vector = &provider.fetch_embedding(query_sample).await?.embedding[0];
    // let query_vector = generate_random_vectors(1, dimension)[0].clone();
    // println!("\nQuery: {}", sample_query.to_string().yellow());
    println!("Querying vector: {:?}...", &query_vector.as_slice()[..3]);
    let top_k = 100;

    let start = std::time::Instant::now();
    let results = hnsw.search_with_metadata(query_vector, top_k, None);
    let search_time = start.elapsed().as_secs_f64();
    println!("Search completed in: {}s", search_time.to_string().yellow());

    println!("\nTop {} nearest neighbors:", top_k);

    for (i, (external_id, similarity, metadata)) in results.iter().take(5).enumerate() {
        println!(
            "  {}. Node {} - similarity: {:.4}, Metadata: {}",
            i + 1,
            external_id.yellow(),
            similarity.to_string().green(),
            metadata.dimmed().green()
        );
    }

    let start_brute = std::time::Instant::now();
    let brute_results = hnsw.brute_force_search_with_metadata(query_vector, top_k);
    let brute_time = start_brute.elapsed().as_secs_f64();
    println!(
        "\nBrute-force search completed in: {}s",
        brute_time.to_string().yellow()
    );

    println!("\nTop {} nearest neighbors (Brute-force):", top_k);

    for (i, (external_id, similarity, metadata)) in brute_results.iter().take(5).enumerate() {
        println!(
            "  {}. Node {} - similarity: {:.4}, Metadata: {}",
            i + 1,
            external_id.yellow(),
            similarity.to_string().green(),
            metadata.dimmed().green()
        );
    }

    let speedup = brute_time / search_time;
    println!("\nSpeedup over brute-force: {:.2}x", speedup);

    verify_bidirectional_edges(&hnsw)?;

    // Choose random nodes from [vectors] to delete_node_by_id
    let delete_count = 15000;

    let mut rng = rand::rng();
    let mut deleted_ids = HashSet::new();
    let mut deleted_vectors: Vec<Node> = Vec::new();

    while deleted_vectors.len() < delete_count {
        let random_index = rng.random_range(0..hnsw.nodes.len());
        if let Some(node) = hnsw.nodes.get(random_index)
            && !node.tombstone
            && !deleted_ids.contains(&node.node_id)
        {
            deleted_ids.insert(node.node_id.clone());
            deleted_vectors.push(node.clone());
        }
    }

    // Remove the top results from the brute-force search to ensure we are deleting nodes that are actually in the graph and likely to be returned in search results
    // let deleted_ids = brute_results
    //     .iter()
    //     .map(|(id, _, _)| id)
    //     .cloned()
    //     .collect::<Vec<String>>();

    let delete_time = std::time::Instant::now();
    for node in &deleted_ids {
        hnsw.delete_node_by_id(node)?;
    }
    println!("Deletion completed in {:?}", delete_time.elapsed());

    verify_graph_connectivity(&hnsw)?;

    verify_bidirectional_edges(&hnsw)?;

    verify_max_neighbors_constraint(&hnsw)?;

    verify_layer_statistics(&hnsw)?;

    let active_nodes = hnsw.active_count();
    let tombstoned_nodes = hnsw.tombstone_count();

    assert_eq!(
        active_nodes + tombstoned_nodes,
        hnsw.nodes.len(),
        "Active + Tombstoned nodes should equal total nodes"
    );

    println!("Tombstone ratio: {}%", hnsw.tombstone_ratio() * 100.0);

    let search_results = hnsw.search(query_vector, 100, None);
    let deleted_in_results = search_results
        .iter()
        .filter(|(external_id, _)| deleted_ids.contains(external_id))
        .count();
    assert_eq!(
        deleted_in_results, 0,
        "Found {} deleted nodes in search results!",
        deleted_in_results
    );

    let expected_active = node_count - deleted_ids.len();
    assert_eq!(
        active_nodes, expected_active,
        "Expected {} active nodes, found {}",
        expected_active, active_nodes
    );

    assert_eq!(
        tombstoned_nodes,
        deleted_ids.len(),
        "Expected {} tombstones, found {}",
        deleted_ids.len(),
        tombstoned_nodes
    );

    if let Some(entry_id) = hnsw.entry_point {
        assert!(
            !hnsw.nodes[entry_id].tombstone,
            "Entry point {} is tombstoned!",
            entry_id
        );
    } else {
        panic!("Entry point is None after deletions!");
    }

    let reindex_time = std::time::Instant::now();
    hnsw.reindex()?;
    println!("Reindexing completed in {:?}", reindex_time.elapsed());

    verify_graph_connectivity(&hnsw)?;

    verify_neighbor_validity(&hnsw)?;

    verify_bidirectional_edges(&hnsw)?;

    verify_max_neighbors_constraint(&hnsw)?;

    verify_layer_statistics(&hnsw)?;

    let post_reindex_tombstones = hnsw.tombstone_count();
    assert_eq!(
        post_reindex_tombstones, 0,
        "Found {} tombstones after reindex!",
        post_reindex_tombstones
    );

    let post_reindex_count = hnsw.nodes.len();
    assert_eq!(
        post_reindex_count, expected_active,
        "Expected {} nodes after reindex, found {}",
        expected_active, post_reindex_count
    );

    assert!(
        hnsw.entry_point.is_some(),
        "Entry point is None after reindex!"
    );
    let entry_id = hnsw.entry_point.unwrap();
    assert!(
        entry_id < hnsw.nodes.len(),
        "Entry point {} is out of bounds!",
        entry_id
    );

    let post_reindex_results = hnsw.search(query_vector, top_k, None);
    assert_eq!(
        post_reindex_results.len(),
        top_k,
        "Expected {} results, got {}",
        top_k,
        post_reindex_results.len()
    );

    let quality_check = compare_search_quality(&hnsw, query_vector, top_k)?;
    println!(
        "Search quality maintained: {:.2}% of brute-force quality",
        quality_check * 100.0
    );

    let start = std::time::Instant::now();
    let results = hnsw.search_with_metadata(query_vector, top_k, None);
    let search_time = start.elapsed().as_secs_f64();
    println!("Search completed in: {}s", search_time.to_string().yellow());

    println!("\nTop {} nearest neighbors:", top_k);

    for (i, (external_id, similarity, metadata)) in results.iter().take(5).enumerate() {
        println!(
            "  {}. Node {} - similarity: {:.4}, Metadata: {}",
            i + 1,
            external_id.yellow(),
            similarity.to_string().green(),
            metadata.dimmed().green()
        );
    }

    let start_brute = std::time::Instant::now();
    let brute_results = hnsw.brute_force_search_with_metadata(query_vector, top_k);
    let brute_time = start_brute.elapsed().as_secs_f64();
    println!(
        "\nBrute-force search completed in: {}s",
        brute_time.to_string().yellow()
    );

    println!("\nTop {} nearest neighbors (Brute-force):", top_k);

    for (i, (external_id, similarity, metadata)) in brute_results.iter().take(5).enumerate() {
        println!(
            "  {}. Node {} - similarity: {:.4}, Metadata: {}",
            i + 1,
            external_id.yellow(),
            similarity.to_string().green(),
            metadata.dimmed().green()
        );
    }

    let speedup = brute_time / search_time;
    println!("\nSpeedup over brute-force: {:.2}x", speedup);

    let quality_check = compare_search_quality(&hnsw, query_vector, top_k)?;
    println!(
        "Search quality maintained: {:.2}% of brute-force quality",
        quality_check * 100.0
    );

    Ok(())
}

fn print_layer_stats(hnsw: &Hnsw) {
    let mut layer_counts = vec![0usize; hnsw.max_layers];

    for node in &hnsw.nodes {
        for layer in 0..=node.max_level {
            if layer < layer_counts.len() {
                layer_counts[layer] += 1;
            }
        }
    }

    println!("\nHnsw Layer Statistics:");
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
async fn get_level_math_debug(hnsw: &Hnsw) -> Result<()> {
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
        let count = random_levels.get(level).unwrap();
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
        for level in 1..levels.len() {
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
#[inline]
/// Verify that the graph is still connected (all active nodes are reachable from entry point)
fn verify_graph_connectivity(hnsw: &Hnsw) -> Result<()> {
    if hnsw.entry_point.is_none() {
        return Err(anyhow::anyhow!("No entry point found"));
    }

    let mut visited = HashSet::new();
    let mut to_visit = vec![hnsw.entry_point.unwrap()];

    while let Some(node_id) = to_visit.pop() {
        if visited.contains(&node_id) || hnsw.nodes[node_id].tombstone {
            continue;
        }

        visited.insert(node_id);

        // Add all neighbors at all layers
        for layer in 0..=hnsw.nodes[node_id].max_level {
            for &neighbor_id in &hnsw.nodes[node_id].neighbors[layer] {
                if !visited.contains(&neighbor_id) && !hnsw.nodes[neighbor_id].tombstone {
                    to_visit.push(neighbor_id);
                }
            }
        }
    }

    let reachable_count = visited.len();
    let active_count = hnsw.active_count();

    // It's okay if not all nodes are reachable
    // but we should have at least some reasonable connectivity
    if reachable_count < active_count / 2 {
        return Err(anyhow::anyhow!(
            "Poor connectivity: only {} out of {} active nodes are reachable",
            reachable_count,
            active_count
        ));
    }

    Ok(())
}
#[inline]
/// Verify that all neighbor references are valid (no out-of-bounds, no tombstones)
fn verify_neighbor_validity(hnsw: &Hnsw) -> Result<()> {
    for (node_idx, node) in hnsw.nodes.iter().enumerate() {
        if node.tombstone {
            return Err(anyhow::anyhow!(
                "Found tombstoned node {} after reindex!",
                node_idx
            ));
        }

        for (layer, neighbors) in node.neighbors.iter().enumerate() {
            for &neighbor_id in neighbors {
                // Check bounds
                if neighbor_id >= hnsw.nodes.len() {
                    return Err(anyhow::anyhow!(
                        "Node {} has out-of-bounds neighbor {} at layer {}",
                        node_idx,
                        neighbor_id,
                        layer
                    ));
                }

                // Check not tombstoned
                if hnsw.nodes[neighbor_id].tombstone {
                    return Err(anyhow::anyhow!(
                        "Node {} has tombstoned neighbor {} at layer {}",
                        node_idx,
                        neighbor_id,
                        layer
                    ));
                }

                // Check neighbor exists at this layer
                if layer > hnsw.nodes[neighbor_id].max_level {
                    return Err(anyhow::anyhow!(
                        "Node {} references neighbor {} at layer {}, but neighbor's max_level is {}",
                        node_idx,
                        neighbor_id,
                        layer,
                        hnsw.nodes[neighbor_id].max_level
                    ));
                }
            }
        }
    }

    Ok(())
}
#[inline]
/// Verify that edges are bidirectional (if A->B then B->A)
fn verify_bidirectional_edges(hnsw: &Hnsw) -> Result<()> {
    for (node_idx, node) in hnsw.nodes.iter().enumerate() {
        for (layer, neighbors) in node.neighbors.iter().enumerate() {
            for &neighbor_id in neighbors {
                // Check if neighbor exists at this layer
                if layer > hnsw.nodes[neighbor_id].max_level {
                    return Err(anyhow::anyhow!(
                        "Node {} has neighbor {} at layer {}, but neighbor's max_level is {}",
                        node_idx,
                        neighbor_id,
                        layer,
                        hnsw.nodes[neighbor_id].max_level
                    ));
                }

                // Check if neighbor has edge back to this node
                let has_reverse_edge = hnsw.nodes[neighbor_id].neighbors[layer].contains(&node_idx);

                if !has_reverse_edge {
                    return Err(anyhow::anyhow!(
                        "Unidirectional edge found: {} -> {} at layer {}, but not {} -> {}",
                        node_idx,
                        neighbor_id,
                        layer,
                        neighbor_id,
                        node_idx
                    ));
                }
            }
        }
    }

    Ok(())
}

#[inline]
/// Verify that max_neighbors constraint is respected
fn verify_max_neighbors_constraint(hnsw: &Hnsw) -> Result<()> {
    for (node_idx, node) in hnsw.nodes.iter().enumerate() {
        for (layer, neighbors) in node.neighbors.iter().enumerate() {
            if neighbors.len() > hnsw.max_neighbors {
                return Err(anyhow::anyhow!(
                    "Node {} has {} neighbors at layer {}, exceeds max_neighbors={}",
                    node_idx,
                    neighbors.len(),
                    layer,
                    hnsw.max_neighbors
                ));
            }

            // Check for duplicate neighbors
            let unique_neighbors: HashSet<_> = neighbors.iter().collect();
            if unique_neighbors.len() != neighbors.len() {
                return Err(anyhow::anyhow!(
                    "Node {} has duplicate neighbors at layer {}",
                    node_idx,
                    layer
                ));
            }

            // Check for self-loops
            if neighbors.contains(&node_idx) {
                return Err(anyhow::anyhow!(
                    "Node {} has a self-loop at layer {}",
                    node_idx,
                    layer
                ));
            }
        }
    }

    Ok(())
}

#[inline]
/// Compare search results to verify quality is maintained
fn compare_search_quality(hnsw: &Hnsw, query: &[f32], k: usize) -> Result<f32> {
    // Get Hnsw results
    let hnsw_results = hnsw.search(query, k, None);

    if hnsw_results.is_empty() {
        return Err(anyhow::anyhow!("Hnsw search returned no results"));
    }

    // Get ground truth 💀
    let brute_results = hnsw.brute_force_search(query, k);

    if brute_results.is_empty() {
        return Err(anyhow::anyhow!("Brute force search returned no results"));
    }

    let hnsw_avg_sim: f32 =
        hnsw_results.iter().map(|(_, sim)| sim).sum::<f32>() / hnsw_results.len() as f32;
    let brute_avg_sim: f32 =
        brute_results.iter().map(|(_, sim)| sim).sum::<f32>() / brute_results.len() as f32;

    let quality_ratio = hnsw_avg_sim / brute_avg_sim;

    // Allow 50% degradation in average similarity after reindex (since we might have removed some nodes)
    if quality_ratio < 0.5 {
        return Err(anyhow::anyhow!(
            "Poor search quality after reindex: Hnsw avg similarity {:.4} vs brute force {:.4} (ratio: {:.2}%)",
            hnsw_avg_sim,
            brute_avg_sim,
            quality_ratio * 100.0
        ));
    }

    Ok(quality_ratio)
}

#[inline]
/// Verify layer statistics are reasonable
fn verify_layer_statistics(hnsw: &Hnsw) -> Result<()> {
    if hnsw.nodes.is_empty() {
        return Ok(());
    }

    let mut layer_counts = vec![0usize; hnsw.max_layers];

    for (idx, node) in hnsw.nodes.iter().enumerate() {
        if node.max_level >= hnsw.max_layers {
            return Err(anyhow::anyhow!(
                "Node {} (index {}) has max_level {} which exceeds max_layers {}",
                node.node_id,
                idx,
                node.max_level,
                hnsw.max_layers
            ));
        }

        for count in layer_counts.iter_mut().take(node.max_level + 1) {
            *count += 1;
        }
    }

    // Verify layer 0 has all nodes
    if layer_counts[0] != hnsw.nodes.len() {
        return Err(anyhow::anyhow!(
            "Layer 0 should have all {} nodes, but has {}",
            hnsw.nodes.len(),
            layer_counts[0]
        ));
    }

    // Verify layers get progressively smaller (with some tolerance)
    for layer in 1..hnsw.max_layers {
        if layer_counts[layer] > layer_counts[layer - 1] {
            return Err(anyhow::anyhow!(
                "Layer {} has more nodes ({}) than layer {} ({})",
                layer,
                layer_counts[layer],
                layer - 1,
                layer_counts[layer - 1]
            ));
        }

        // Stop checking if we've reached empty layers
        if layer_counts[layer] == 0 {
            break;
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
        "Hnsw Debug Log \nStarted at: {}\n{}\n",
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
