# Blaze-DB

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and retrieval of vector
embeddings.

## Current State

- Reads text data from a specified file.
- Chunks the data into smaller batches (configurable batch sizes).
- Generates vector embeddings for each batch using Ollama API.
- Stores the generated embeddings on disk in binary format for optimal performance.
- Fast parallel loading of embeddings from disk using memory-mapped files.
- **Semantic similarity search with multiple distance metrics** (Cosine, Euclidean, Dot Product).
- Top-K result retrieval with ranked scoring.
- Async/await architecture for non-blocking operations.
- Parallel processing with Rayon for compute-intensive operations.
- Performance benchmarking suite (~3.7ms per search on War and Peace dataset).
- CLI for server and client for talking to http database server.

### DEMO

```shell
Chunk: There is no Peace without War,
Wars should be celebrated,
Because it is the win against the evil.
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

## Roadmap

- HNSW (Hierarchical Navigable Small World) indexing for improved search performance.
- Product quantization for memory optimization.
- HTTP API server for remote database access.
- Query filtering and metadata support.
- CLI client for database management.
- Incremental updates without full reindex.
- Distributed storage and sharding support.

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️