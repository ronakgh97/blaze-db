# Blaze-DB

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and fast retrieval of vector
embeddings.

## Current State

- Two binaries: `blaze-server` and `blaze-client`, for server and client operations respectively.
- Uses Ollama API for generating vector embeddings.
- Batch/Chunks processing for embedding generation.
- Stores embeddings on disk in binary format.
- Use memory-mapped files for fast loading of embeddings, rayon for parallel processing.
- Uses semantic similarity search with multiple distance metrics (Cosine, Euclidean, Dot Product).
- Async/await architecture for non-blocking operations.
- Performance benchmarking suite (~3.7ms per search on War and Peace dataset).

### DEMO

```shell
Chunk: There is no Peace without War, Wars should be celebrated, Because it is the win against the evil.
Embedding (First 3): [0.04979933, -0.06230091, -0.009091219]
Found 102 binary files to load...

Top 5 similar chunks:

Result 1:
Chunk: of War needlessly deviating.”
Score: 0.6425

Result 2:
Chunk: that: get rid of falsehood and let war be war and not a game. As it is
Score: 0.6276

Result 3:
Chunk: without knowing why. War is not courtesy but the most horrible thing in
Score: 0.6204

Result 4:
Chunk: *** END OF THE PROJECT GUTENBERG EBOOK WAR AND PEACE ***
Score: 0.6202

Result 5:
Chunk: *** START OF THE PROJECT GUTENBERG EBOOK WAR AND PEACE ***
Score: 0.6123
Search took: 84.1797ms for 51788 vectors
```

### NSW DEMO

```shell

Building NSW graph with 50000 nodes...

Rearranged in 106.8534603s

Graph Analysis:
Total Nodes: 50000
Total Edges: 800000
Average Edges per Node: 16.00
Nodes with most neighbour count: 50000

Querying vector: [1.1767917, -1.995266, 0.7625747]...

Greedy search completed in 0.0000403s

Top 5 Greedy Search Results:
Result 1: Node Index: 32477, Similarity: 0.09
Result 2: Node Index: 30343, Similarity: 0.03
Result 3: Node Index: 23980, Similarity: -0.0

Parallel Greedy search completed in 0.0010476s

Top 5 Parallel Greedy Search Results:
Result 1: Node Index: 41269, Similarity: 0.11
Result 2: Node Index: 4418, Similarity: 0.10
Result 3: Node Index: 4058, Similarity: 0.10
Result 4: Node Index: 29716, Similarity: 0.10
Result 5: Node Index: 37193, Similarity: 0.10

Brute search completed in 0.3269939...

Top 5 Brute-force Results:
Result 1: Node Index: 31738, Similarity: 0.13
Result 2: Node Index: 47913, Similarity: 0.13
Result 3: Node Index: 41260, Similarity: 0.12
Result 4: Node Index: 3757, Similarity: 0.12
Result 5: Node Index: 33942, Similarity: 0.11
```

## TODO:

- HNSW (Hierarchical Navigable Small World) indexing for improved search performance.
- Complete HTTP API server for remote database access.
- Query filtering and metadata support.
- Incremental updates without full reindex.
- Distributed storage and sharding support.
- Move hardcoded Values to separate config files.
- HNSW DEMO and benchmarking.
- Cloud deployment options.

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️