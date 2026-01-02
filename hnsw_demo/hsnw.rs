use rand::Rng;

#[derive(Debug, Clone)]
struct HNSW {
    pub nodes: Vec<Node>,
}

impl HNSW {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn rearrange_nodes(&mut self) {
        unimplemented!("THE CORE HNSW ALGORITHM IS NOT IMPLEMENTED YET");
    }
}

#[derive(Debug, Clone)]
struct Node {
    pub vector: [f32; 3], // Example with 3 dimensions
    pub neighbors: Option<Vec<Box<Node>>>,
}

fn main() {
    println!("HSNW Demo");

    let mut hnsw = HNSW::new();

    // Generate 100K random vectors
    let num_vectors = 100_000;
    for i in 0..num_vectors {
        let vector = generate_random_vector();
        // Create a node with none neighbors for simplicity
        let node = Node {
            vector,
            neighbors: None,
        };
        hnsw.add_node(node);
    }
}

fn generate_random_vector() -> [f32; 3] {
    let mut rng = rand::rng();
    [
        rng.random_range(0.0..1.0),
        rng.random_range(0.0..1.0),
        rng.random_range(0.0..1.0),
    ]
}
