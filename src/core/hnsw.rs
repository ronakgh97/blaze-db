use crate::core::{Metrics, cosine_similarity, dot_product, euclidean_similarity};
use anyhow::Result;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelIterator;
#[allow(unused)]
use rayon::prelude::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use wincode::{SchemaRead, SchemaWrite};

/// Priority queue entry for nodes to explore during search.
/// Example: candidates with similarities [0.5, 0.9, 0.3]
/// - Heap top will be 0.9 (highest)
/// - pop() returns 0.9, then 0.5, then 0.3
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
/// Example: results with similarities [0.9, 0.7, 0.5] (k=3)
/// - Heap top will be 0.5 (lowest in our top-k)
/// - When new candidate with 0.8 arrives:
/// - 0.8 > 0.5 (worst), so we add it and pop the worst (0.5)
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

const DEFAULT_EF_INC_FACTOR: f32 = 1.57575;

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
/// **Search**: Greedy descent through upper layers, bounded search at bottom layer
/// **Pruning**: Keep only M closest neighbors per node per layer
/// **Tombstones**: Mark deleted nodes and skip during search, periodic cleanup

#[derive(Serialize, Deserialize, Debug, Clone, SchemaWrite, SchemaRead)]
pub struct HNSW {
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
    /// Mapping from node ID (set by params) to array index
    /// It's just a fucking HashMap for O(1) lookups, that's all it is, nothing fancy
    id_mapper: HashMap<NodeID, NodeIndex>,
}

impl Default for HNSW {
    fn default() -> Self {
        HNSW::new(18, 256, 16, 1.0, &Some(Metrics::Cosine))
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
    ///
    /// # Arguments
    /// * `node_id` - User-provided node ID (must be unique)
    /// * `vector` - The vector to insert
    /// * `metadata` - Metadata associated with the node
    /// * `max_level` - The maximum level for this node
    ///
    /// # Returns
    /// * `Ok(NodeId)` - The array index of the newly inserted node in the nodes vector
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
            return Err(anyhow::anyhow!("node ID '{}' already exists", vector_id));
        }

        let node_id = self.nodes.len();

        // Create the node with empty neighbor lists (we'll fill them in after finding neighbors)
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

        // Register the node ID mapping for O(1) lookups
        self.id_mapper.insert(vector_id, node_id);

        if self.entry_point.is_none() {
            self.nodes.push(node);
            self.entry_point = Some(node_id);
            return Ok(node_id);
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

            let selected: Vec<NodeIndex> = candidates
                .into_par_iter()
                .take(self.max_neighbors)
                .collect();

            // Connect new node to its neighbors (bidirectional!!)
            for &neighbor_id in &selected {
                if layer <= self.nodes[neighbor_id].max_level {
                    self.nodes[node_id].neighbors[layer].push(neighbor_id);
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

        if max_level > entry_level {
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

        while improved {
            improved = false;

            // Check all neighbors at this layer
            if layer <= self.nodes[current].max_level {
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
                    }
                }
            }
        }

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

        while let Some(Candidate(current_id, current_sim)) = candidates.pop() {
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
                for &neighbor_id in &self.nodes[current_id].neighbors[layer] {
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

                            if results.len() > ef {
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

        sorted_results
            .into_iter()
            .map(|ScoredResult(id, _)| id)
            .collect()
    }

    /// Remove connections to keep only the M closest neighbors
    fn prune_connections(&mut self, node_id: NodeIndex, layer: usize) {
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
    /// * `ef_search` - Optional bounded width for search. If None, uses k * DEFAULT_EF_MULTIPLIER
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

            // Filter out tombstoned nodes and convert to node IDs
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

    /// Internal search method that performs the actual HNSW search
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

    /// Delete a node by node ID
    /// If the deleted node is the entry point, finds a new one
    #[inline]
    pub fn delete_node_by_id(&mut self, node_id: &str) -> Result<()> {
        let node_id = self
            .id_mapper
            .get(node_id)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("node ID '{}' not found", node_id))?;

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

    /// Rebuilds the HNSW index by removing all tombstoned nodes
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

        // Update node ID mapping
        self.id_mapper = new_id_mapper;

        Ok(())
    }
}

/// This is changes during reindexing
/// It's just a fucking array index, that ip_mapper in HNSW? It's just for fucking O(1)
pub type NodeIndex = usize;

/// Unique identifier for a node. (Stable across reindexing)
pub type NodeID = String;

#[derive(Serialize, Deserialize, Debug, Clone, SchemaWrite, SchemaRead)]
/// Represents a node in the HNSW graph.
pub struct Node {
    /// node identifier - stable across reindexing
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
