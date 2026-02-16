use crate::core::{Metrics, cosine_similarity, dot_product, euclidean_similarity};
use rayon::iter::IndexedParallelIterator;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelIterator;
#[allow(unused)]
use rayon::prelude::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wincode::{SchemaRead, SchemaWrite};

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
/// **Tombstones (TODO)**: Mark deleted nodes and skip during search, periodic cleanup

#[derive(Serialize, Deserialize, Debug, Clone, SchemaWrite, SchemaRead)]
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
    /// Similarity metric to use for distance calculations (default: Cosine)
    // TODO: Take Option type here
    pub metrics: Option<Metrics>,
}

impl Default for HNSW {
    /// Default HNSW parameters:
    /// - max_neighbors: 16
    /// - ef_construction: 200
    /// - max_layers: 12
    /// - distribution_bias: 1.0 (Unused btw)
    fn default() -> Self {
        HNSW::new(16, 200, 16, 1.0, &Some(Metrics::Cosine))
    }
}

impl HNSW {
    /// Creates a new HNSW instance with specified parameters.
    pub fn new(
        max_neighbors: usize,
        ef_construction: usize,
        max_layers: usize,
        distribution_bias: f32,
        metrics: &Option<Metrics>,
    ) -> Self {
        HNSW {
            nodes: Vec::with_capacity(10_000), // Preallocate for efficiency
            entry_point: None,
            max_layers,
            max_neighbors,
            ef_construction,
            distribution_bias, // Currently unused
            metrics: metrics.clone(),
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
            return node_id;
        }

        let new_vector = node.vector.clone();
        self.nodes.push(node);

        // Start search from entry point
        let mut current_nearest = self.entry_point.expect("Ohh no...entry_point is None");
        let entry_level = self.nodes[current_nearest].max_level;

        // Greedily traverse from top layer down to new node's level + 1
        // Just find the closest node, don't connect yet
        for layer in (max_level + 1..=entry_level).rev() {
            current_nearest = self.search_layer_greedy(&new_vector, current_nearest, layer);
        }

        // From new node's max_level down to 0, find neighbors and connect
        for layer in (0..=max_level).rev() {
            // Find ef_construction nearest neighbors at this layer
            let candidates =
                self.search_layer_knn(&new_vector, current_nearest, self.ef_construction, layer);

            // But only connect to max_neighbors of them
            let selected: Vec<NodeId> = candidates
                .into_par_iter()
                .take(self.max_neighbors)
                .collect();

            // Connect new node to its neighbors (bidirectional)
            for &neighbor_id in &selected {
                self.nodes[node_id].neighbors[layer].push(neighbor_id);

                if layer <= self.nodes[neighbor_id].max_level {
                    self.nodes[neighbor_id].neighbors[layer].push(node_id);

                    // Prune neighbor's connections if it has too many
                    if self.nodes[neighbor_id].neighbors[layer].len() > self.max_neighbors {
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
            self.entry_point = Some(node_id);
        }

        node_id
    }

    /// Greedy search: find single closest node at a layer
    /// Used for navigating upper layers quickly
    fn search_layer_greedy(&self, query: &[f32], entry: NodeId, layer: usize) -> NodeId {
        let mut current = entry;
        let mut current_sim =
            self.similarity(query, &self.nodes[current].vector, self.metrics.as_ref());
        let mut improved = true;

        while improved {
            improved = false;

            // Check all neighbors at this layer
            if layer <= self.nodes[current].max_level {
                for &neighbor_id in &self.nodes[current].neighbors[layer] {
                    let neighbor_sim = self.similarity(
                        query,
                        &self.nodes[neighbor_id].vector,
                        self.metrics.as_ref(),
                    );

                    if neighbor_sim > current_sim {
                        current = neighbor_id;
                        current_sim = neighbor_sim;
                        improved = true;
                    }
                }
            }
        }

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
        let mut visited = HashSet::with_capacity(self.nodes.len());
        // Working set (to explore)
        let mut candidates = Vec::with_capacity(10000); // Preallocate
        // Best found so far
        let mut results = Vec::with_capacity(candidates.capacity());

        let entry_sim = self.similarity(query, &self.nodes[entry].vector, self.metrics.as_ref());
        visited.insert(entry);
        candidates.push((entry, entry_sim));
        results.push((entry, entry_sim));

        // Bounded beam search - only explore ef best candidates
        while let Some((current_id, current_sim)) = candidates.pop() {
            // Pruning: if current is worse than worst result, skip
            if !results.is_empty() {
                results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Higher similarity first
                if results.len() >= ef && current_sim < results[ef - 1].1 {
                    continue; // Skip this candidate
                }
            }

            // Explore neighbors
            if layer <= self.nodes[current_id].max_level {
                for &neighbor_id in &self.nodes[current_id].neighbors[layer] {
                    if visited.insert(neighbor_id) {
                        let sim = self.similarity(
                            query,
                            &self.nodes[neighbor_id].vector,
                            self.metrics.as_ref(),
                        );

                        // Only add if better than worst result or we haven't found ef results yet
                        if results.len() < ef || sim > results[ef - 1].1 {
                            candidates.push((neighbor_id, sim));
                            results.push((neighbor_id, sim));

                            // Keep results sorted and bounded
                            if results.len() > ef {
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

        results.into_par_iter().map(|(id, _)| id).collect()
    }

    /// Remove connections to keep only the M closest neighbors
    fn prune_connections(&mut self, node_id: NodeId, layer: usize) {
        // Store the old neighbor list to identify which edges to remove
        let old_neighbors: HashSet<NodeId> = self.nodes[node_id].neighbors[layer]
            .iter()
            .copied()
            .collect();

        // Calculate similarities to all neighbors
        let mut neighbor_sims: Vec<(NodeId, f32)> = self.nodes[node_id].neighbors[layer]
            .par_iter()
            .map(|&n| {
                let sim = self.similarity(
                    &self.nodes[node_id].vector,
                    &self.nodes[n].vector,
                    self.metrics.as_ref(),
                );
                (n, sim)
            })
            .collect();

        // Keep only the M most similar
        neighbor_sims.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        neighbor_sims.truncate(self.max_neighbors);

        let new_neighbors: Vec<NodeId> = neighbor_sims.into_par_iter().map(|(id, _)| id).collect();
        let new_neighbors_set: HashSet<NodeId> = new_neighbors.iter().copied().collect();

        // Find nodes that were removed (old - new)
        let removed_neighbors: Vec<NodeId> = old_neighbors
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
    /// Returns results as (NodeId, similarity) tuples sorted by similarity (highest first)
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)> {
        if self.entry_point.is_none() {
            return Vec::new();
        }

        let entry = self.entry_point.unwrap();
        let entry_level = self.nodes[entry].max_level;
        let mut current = entry;

        // Traverse from top to layer 1
        for layer in (1..=entry_level).rev() {
            current = self.search_layer_greedy(query, current, layer);
        }

        let candidates = self.search_layer_knn(query, current, k * 2, 0);

        // Return with similarities
        let mut results: Vec<(NodeId, f32)> = candidates
            .into_par_iter()
            .map(|id| {
                (
                    id,
                    self.similarity(query, &self.nodes[id].vector, self.metrics.as_ref()),
                )
            })
            .collect();

        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Higher similarity first
        results.truncate(k);

        results
    }

    #[inline]
    /// Search and return results with metadata
    /// Returns results as (NodeId, similarity, metadata) tuples sorted by similarity (highest first)
    pub fn search_with_metadata(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32, &str)> {
        let results = self.search(query, k);
        results
            .into_iter() // TODO:  parallel iterator not needed here?
            .map(|(id, sim)| (id, sim, self.nodes[id].metadata.as_str()))
            .collect()
    }

    #[inline]
    pub fn brute_force_search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)> {
        let mut results: Vec<(NodeId, f32)> = self
            .nodes
            .par_iter()
            .enumerate()
            .map(|(id, node)| {
                (
                    id,
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
    ) -> Vec<(NodeId, f32, &str)> {
        let results = self.brute_force_search(query, k);
        results
            .into_iter()
            .map(|(id, sim)| (id, sim, self.nodes[id].metadata.as_str()))
            .collect()
    }

    #[inline]
    /// Get metadata for a specific node
    pub fn get_metadata_by_id(&self, node_id: NodeId) -> Option<&String> {
        self.nodes.get(node_id).map(|node| &node.metadata)
    }

    #[inline]
    /// Get vector for a specific node
    pub fn get_vector_by_id(&self, node_id: NodeId) -> Option<&Vec<f32>> {
        self.nodes.get(node_id).map(|node| &node.vector)
    }

    //TODO: Deletion API - non-trivial in HNSW, requires careful handling of neighbors and layers
    //TODO: Will use Tombstone markers for 'deleted nodes' and skip them during search, then periodically rebuild the graph to clean up 😌
}

/// Unique identifier for a node in the HNSW graph.
pub type NodeId = usize;

#[derive(Serialize, Deserialize, Debug, Clone, SchemaWrite, SchemaRead)]
/// Represents a node in the HNSW graph.
pub struct Node {
    /// Unique identifier for the node
    pub id: NodeId,
    /// Metadata associated with the node
    pub metadata: String, // String for now, I guess? TODO: make generic or JSON VALUE?
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
            neighbors: vec![Vec::new(); max_level + 1],
            max_level,
        }
    }
}
