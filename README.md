# Blaze-DB

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and fast retrieval of
vector
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

### NORMAL SEARCH DEMO

```shell
Chunk: What this book about?
Embedding (First 3): [0.02176302, -0.05591798, -0.007288052]

Top 5 similar chunks:

Result 1:
Chunk: had never grown. This active old servant was unpacking the traveler’s canteen and preparing tea. He brought in a boiling samovar. When everything was ready, the stranger opened his eyes, moved to the table, filled a tumbler with tea for himself and one for the beardless old man to whom he passed it. Pierre began to feel a sense of uneasiness, and the need, even the inevitability, of entering into conversation with this stranger. The servant brought back his tumbler turned upside down, * with an unfinished bit of nibbled sugar, and asked if anything more would be wanted. * To indicate he did not want more tea. “No. Give me the book,” said the stranger. The servant handed him a book which Pierre took to be a devotional work, and the traveler became absorbed in it. Pierre looked at him. All at
Score: 0.3977

Result 2:
Chunk: crossed his gloved hands on his breast, and began to speak. “Now I must disclose to you the chief aim of our Order,” he said, “and if this aim coincides with yours, you may enter our Brotherhood with profit. The first and chief object of our Order, the foundation on which it rests and which no human power can destroy, is the preservation and handing on to posterity of a certain important mystery... which has come down to us from the remotest ages, even from the first man—a mystery on which perhaps the fate of mankind depends. But since this mystery is of such a nature that nobody can know or use it unless he be prepared by long and diligent self-purification, not everyone can hope to attain it quickly. Hence we have a secondary aim, that of preparing
Score: 0.3956

Result 3:
Chunk: that his relations’ choice has fallen. I don’t know what you will think of it, but I consider it my duty to let you know of it. He is said to be very handsome and a terrible scapegrace. That is all I have been able to find out about him. But enough of gossip. I am at the end of my second sheet of paper, and Mamma has sent for me to go and dine at the Apráksins’. Read the mystical book I am sending you; it has an enormous success here. Though there are things in it difficult for the feeble human mind to grasp, it is an admirable book which calms and elevates the soul. Adieu! Give my respects to monsieur your father and my compliments to Mademoiselle Bourienne. I embrace you as I love you. JULIE
Score: 0.3902

Result 4:
Chunk: what I desire most on earth, it would be to be poorer than the poorest beggar. A thousand thanks, dear friend, for the volume you have sent me and which has such success in Moscow. Yet since you tell me that among some good things it contains others which our weak human understanding cannot grasp, it seems to me rather useless to spend time in reading what is unintelligible and can therefore bear no fruit. I never could understand the fondness some people have for confusing their minds by dwelling on mystical books that merely awaken their doubts and excite their imagination, giving them a bent for exaggeration quite contrary to Christian simplicity. Let us rather read the Epistles and Gospels. Let us not seek to penetrate what mysteries they contain; for how can we, miserable sinners that we are, know the
Score: 0.3849

Result 5:
Chunk: satisfied with this answer. “Have you sought for means of attaining your aim in religion?” “No, I considered it erroneous and did not follow it,” said Pierre, so softly that the Rhetor did not hear him and asked him what he was saying. “I have been an atheist,” answered Pierre. “You are seeking for truth in order to follow its laws in your life, therefore you seek wisdom and virtue. Is that not so?” said the Rhetor, after a moment’s pause. “Yes, yes,” assented Pierre. The Rhetor cleared his throat, crossed his gloved hands on his breast, and began to speak. “Now I must disclose to you the chief aim of our Order,” he said, “and if this aim coincides with yours, you may enter our Brotherhood with profit. The first and chief object of our Order, the foundation on
Score: 0.3800

I/O took: 11.4964ms for 5981 vectors
Search took: 3.2123ms for 5981 vectors
Total took: 15.7322ms
```

### NSW DEMO WITH BENCHMARKING

```shell
Building NSW graph with 50000 nodes...
Rearranged in 152.7613933s

Querying vector: [0.8538208, 0.9682727, 0.5688729]...

Parallel Greedy search with 5 start points, completed in 0.0002725s

Top 5 Parallel Greedy Search Results:
Result 1: Node Index: 49687, Similarity: 0.10
Result 2: Node Index: 33600, Similarity: 0.08
Result 3: Node Index: 1301, Similarity: 0.07
Result 4: Node Index: 46925, Similarity: 0.07
Result 5: Node Index: 27158, Similarity: 0.06

Brute Force search completed in 0.061732s

Top 5 Brute-force Results:
Result 1: Node Index: 40173, Similarity: 0.13
Result 2: Node Index: 40968, Similarity: 0.12
Result 3: Node Index: 11221, Similarity: 0.12
Result 4: Node Index: 34480, Similarity: 0.11
Result 5: Node Index: 8626, Similarity: 0.11
```

- Almost 200x speedup with NSW over parallel brute-force search on 50k vectors!
- Beware: These are very random, high-dimensional vectors, so accuracy may be low, since finding true nearest neighbors
  in high dimensions is inherently difficult (curse of dimensionality).

### HHSW DEMO WITH BENCHMARKING

```shell
Building HNSW graph with 50000 nodes...                                                                                 Indexing completed in 291.6110926s
Indexing completed in 291.6110926s

HNSW Layer Statistics:
  Layer 0: 50000 nodes (100.00%)
  Layer 1: 3070 nodes (6.14%)
  Layer 2: 183 nodes (0.37%)
  Layer 3: 16 nodes (0.03%)
  Layer 4: 3 nodes (0.01%)
  Entry point: node 18377 at layer 4

Performing search...
Search took: 0.0005666s

Top 10 nearest neighbors:
  1. Node 36602 - similarity: 0.10
  2. Node 11926 - similarity: 0.09
  3. Node 35536 - similarity: 0.08
  4. Node 18359 - similarity: 0.08
  5. Node 15308 - similarity: 0.08
  6. Node 16649 - similarity: 0.07
  7. Node 8112  - similarity: 0.07
  8. Node 3205  - similarity: 0.07
  9. Node 1895  - similarity: 0.07
  10. Node 6704  - similarity: 0.06
```

- Again, significant speedup with HNSW over brute-force search on 50k vectors!
- Curse of dimensionality still applies.
- HNSW implementation is basic and can be further optimized. (Which are beyond of my knowledge 😵‍💫)
- Anyways, Look at that smooth exponential layer distribution! *chief kiss* 😼

## TODO:

- HNSW (Hierarchical Navigable Small World) indexing for improved search performance.
- Fix Chunking for better meaningful text segments.
- Write/insert functionality for adding new vectors to the database. (Currently read-only, or rebuild entire DB)
- Make a storage engine, e.g SSTable or LSMTree based.
- Use gRPC/Protobuf for client-server communication?
- Complete HTTP API server for remote database access.
- Query filtering and metadata support.
- Incremental updates without full reindex. (HNSW)
- Distributed storage and sharding support.
- Move hardcoded Values to separate config files.
- HNSW DEMO and benchmarking.
- Cloud deployment options.

## References

- [Curse of Dimensionality](https://en.wikipedia.org/wiki/Curse_of_dimensionality)

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️