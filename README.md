# Blaze-DB

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and fast retrieval of
embeddings using HNSW Indexing.

## Current State

- Two binaries: `blaze-server` and `blaze-client`, for server and client operations respectively.
- Uses Ollama API for generating vector embeddings or Bring your own model embeddings using API Server.
- Batch/Chunks processing for embedding generation (Only used in CLI Wrapper).
- Stores/Index embeddings on disk in binary/JSON format.
- Use memory-mapped files for fast loading and concurrent reads, rayon for parallel processing (where possible).
- Index caching (LRU), which gives 46x faster I/O with reads and writes lockings (thread-safe).
- Implements HNSW (Hierarchical Navigable Small World) graph for approximate nearest neighbor search.
- Basic HTTP API server for remote database access.
- CLI client for local/remote database querying.
- Uses semantic similarity search with multiple distance metrics (Cosine, Euclidean, Dot Product).
- Performance benchmarking suite (<1ms per search on War and Peace dataset, <1ms per search on Amazon Product Dataset).

## Usage

### Start the Server bin

```shell
blzsrv serve

[14:15:46][INFO] Starting the Server...
[14:15:46][INFO] Source: default_src is valid
[14:15:46][INFO] Source: test_src is valid
[14:15:46][INFO] Starting server with 2 valid source(s)
[14:15:46][INFO] Server is running on http://0.0.0.0:8080
[14:15:46][INFO] Using Sources: ["default_src", "test_src"]
```

- Download the Index
  here: [Google Drive Link](https://drive.google.com/file/d/1rnnpMNYzbwkOr9dIetZW83JeF5WCV5cL/view?usp=sharing)
- Checksum (Sha256): **20e7ec6fb00fc7d6988daa0a67349a76898a44dfd46c899cc841f937f0d429b8**
- Extract to `~/.blaze/sources/default_src/amazon_products_2023/`

### Query using CLI Client

```shell
blzdb create --database amazon_products_2023 --source default_src

blzdb query --database amazon_products_2023 --source default_src --search "Wireless Bluetooth Headphones with Noise Cancellation" --top_k 10
```

## Benchmarks

### SEARCH ON 2023 AMAZON PRODUCT DATASET (278528 Index)

```shell
Query: Gaming RTX 4060 Laptop with 165Hz Display
Search completed in: 4.5457ms
Top 100 search results for query: 'Gaming RTX 4060 Laptop with 165Hz Display'
1. ID: 134406, Score: 0.81
Title: Razer Blade 16 Gaming Laptop: NVIDIA GeForce RTX 4090-13th Gen Intel 24-Core i9 HX CPU - 16" Dual Mode Mini LED (4K UHD+ 120Hz & FHD+ 240Hz) - 32GB RAM - 2TB SSD - Compact GaN Charger - Windows 11
2. ID: 134757, Score: 0.80
Title: Razer Blade 18 Gaming Laptop: NVIDIA GeForce RTX 4090-13th Gen Intel 24-Core i9 HX CPU - 18" QHD+ 240Hz - 32GB RAM - 2TB SSD - CNC Aluminum - Compact GaN Charger - Windows 11 - Chroma RGB
3. ID: 193739, Score: 0.78
Title: ASUS ROG Swift PG32UQR 32” 4K HDR 144Hz DSC HDMI 2.1 Gaming Monitor - UHD (3840 x 2160), IPS, 1ms, G-SYNC Compatible, Extreme Low Motion Blur Sync, Eye Care, DisplayPort, USB, DisplayHDR 600,BLACK
4. ID: 195982, Score: 0.78
Title: Alienware M15 R7 Gaming Laptop, 15.6 inch QHD 240Hz 2ms Display, AMD Ryzen R7 6800H, GeForce RTX 3070Ti, 32GB DDR5 RAM, 1TB NVMe SSD, USB-C, Wi-Fi 6, RGB LED Lighting, Windows 11, Black
5. ID: 195699, Score: 0.78
Title: Dell 24 Inch Gaming Monitor, 1ms response time, Overclocked 144Hz AMD FreeSync
6. ID: 193756, Score: 0.78
Title: "LG 32GK650F-B 32" QHD Gaming Monitor with 144Hz Refresh Rate and Radeon FreeSync Technology", Black
7. ID: 193865, Score: 0.78
Title: Alienware AW2521H 25" Full HD LED LCD Monitor - 16:9
8. ID: 193691, Score: 0.77
Title: ASUS ROG Swift 32” 4K HDR 144Hz DSC Gaming Monitor (PG32UQX) - UHD (3840 x 2160), Mini-LED IPS, G-SYNC Ultimate, Local dimming, Quantum Dot technology, DisplayHDR 1400, Eye Care, DisplayPort, HDMI
9. ID: 193465, Score: 0.77
Title: ASUS ROG Swift 27” 1440P Gaming Monitor (PG279QM) - WQHD, Fast IPS, 240Hz, 1ms, G-SYNC, NVIDIA Reflex Latency Analyzer, DisplayHDR400, Eye Care, HDMI, DisplayPort, USB, Height Adjustable,BLACK
```

- Had a classic moment here, was getting 28ms, until I realized that I was running in debug mode. 😶
- Anyways, 4.5ms is pretty good for 278528 vectors! 👨‍🍳🔥
- Amazon product 2023
  dataset: [Source Link](https://www.kaggle.com/datasets/asaniczka/amazon-products-dataset-2023-1-4m-products?select=amazon_products.csv)

### SEARCH ON WAR AND PEACE DATASET

```shell
Query: War and Peace
Embedding (First 3): [0.024166137, -0.016076643, -0.011579157]

Lastest index file loaded: 5
Checksum: 2a9da1ce3b23bf82e3d01836a91e1128561374bede53ba06ed2d0b165ef45f33
Loaded HNSW index with 5981 nodes
Index parameters: M=18, ef_construction=200, layers=12

Top 5 similar chunks (HNSW):

Node ID: 1
Similarity: 0.56
Vector (first 5): [0.10298232, 0.048900284, -0.007921904, -0.009048831, -0.01950096]
Metadata: Author: graf Leo Tolstoy Translator: Aylmer Maude Louise Maude Release date: April 1, 2001 [eBook #2600] Most recently updated: June 14, 2022 Language: English Credits: An Anonymous Volunteer and David Widger *** START OF THE PROJECT GUTENBERG EBOOK WAR AND PEACE *** WAR AND PEACE By Leo Tolstoy/Tolstoi CHAPTER I “Well, Prince, so Genoa and Lucca are now just family estates of the Buonapartes. But I warn you, if you don’t tell me that this means war, if you still try to defend the infamies and horrors perpetrated by that Antichrist—I really believe he is Antichrist—I will have nothing more to do with you and you are no longer my friend, no longer my ‘faithful slave,’ as you call yourself! But how do you do? I see I have frightened you—sit down and tell me all the news.”

Node ID: 0
Similarity: 0.53
Vector (first 5): [0.036207102, 0.030403586, -0.004078724, -0.037874907, -0.022600956]
Metadata: The Project Gutenberg eBook of War and Peace This ebook is for the use of anyone anywhere in the United States and most other parts of the world at no cost and with almost no restrictions whatsoever. You may copy it, give it away or re-use it under the terms of the Project Gutenberg License included with this ebook or online at www.gutenberg.org. If you are not located in the United States, you will have to check the laws of the country where you are located before using this eBook. Title: War and Peace Author: graf Leo Tolstoy Translator: Aylmer Maude Louise Maude Release date: April 1, 2001 [eBook #2600] Most recently updated: June 14, 2022 Language: English Credits: An Anonymous Volunteer and David Widger *** START OF THE PROJECT GUTENBERG EBOOK WAR AND PEACE *** WAR AND PEACE By Leo Tolstoy/Tolstoi CHAPTER I

Node ID: 983
Similarity: 0.48
Vector (first 5): [0.08092821, -0.03397273, -0.013446445, -0.04074625, -0.0458255]
Metadata: such a nation and would endeavor to be worthy of it. This rescript began with the words: “Sergéy Kuzmích, From all sides reports reach me,” etc. “Well, and so he never got farther than: ‘Sergéy Kuzmích’?” asked one of the ladies. “Exactly, not a hair’s breadth farther,” answered Prince Vasíli, laughing, “‘Sergéy Kuzmích... From all sides... From all sides... Sergéy Kuzmích...’ Poor Vyazmítinov could not get any farther! He began the rescript again and again, but as soon as he uttered ‘Sergéy’ he sobbed, ‘Kuz-mí-ch,’ tears, and ‘From all sides’ was smothered in sobs and he could get no farther. And again his handkerchief, and again: ‘Sergéy Kuzmích, From all sides,’... and tears, till at last somebody else was asked to read it.” “Kuzmích... From all sides... and then tears,” someone repeated laughing. “Don’t be unkind,” cried Anna Pávlovna from her end of the table

Node ID: 51
Similarity: 0.48
Vector (first 5): [0.024561673, -0.021977345, -0.009440806, 0.028978676, -0.02957123]
Metadata: disapproved. “The means are ... the balance of power in Europe and the rights of the people,” the abbé was saying. “It is only necessary for one powerful nation like Russia—barbaric as she is said to be—to place herself disinterestedly at the head of an alliance having for its object the maintenance of the balance of power of Europe, and it would save the world!” “But how are you to get that balance?” Pierre was beginning. At that moment Anna Pávlovna came up and, looking severely at Pierre, asked the Italian how he stood Russian climate. The Italian’s face instantly changed and assumed an offensively affected, sugary expression, evidently habitual to him when conversing with women. “I am so enchanted by the brilliancy of the wit and culture of the society, more especially of the feminine society, in which I have had

Node ID: 4575
Similarity: 0.47
Vector (first 5): [0.04066606, -0.019811766, -0.011853991, 0.023033887, -0.0025452946]
Metadata: Pávlovna whispered the next words in advance, like an old woman muttering the prayer at Communion: “Let the bold and insolent Goliath...” she whispered. Prince Vasíli continued. “Let the bold and insolent Goliath from the borders of France encompass the realms of Russia with death-bearing terrors; humble Faith, the sling of the Russian David, shall suddenly smite his head in his bloodthirsty pride. This icon of the Venerable Sergius, the servant of God and zealous champion of old of our country’s weal, is offered to Your Imperial Majesty. I grieve that my waning strength prevents rejoicing in the sight of your most gracious presence. I raise fervent prayers to Heaven that the Almighty may exalt the race of the just, and mercifully fulfill the desires of Your Majesty.” “What force! What a style!” was uttered in approval both of reader and of author.

I/O took: 32.6335ms to load 5981 nodes
HNSW search took: 393.5µs for 5981 nodes
Total took: 37.3125ms
```

### NSW DEMO WITH BENCHMARKING (RANDOM 50,000 VECTORS)

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

### HHSW DEMO WITH BENCHMARKING (RANDOM 50,000 VECTORS)

```shell
Building HNSW graph with 50000 nodes...
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
- Anyway, Look at that smooth exponential layer distribution! _chief kiss_ 😼

### Cache Benchmarking

```shel
[10:17:18][INFO] Acquired read lock for database 'test_db'
[10:17:18][INFO] Released read lock for database 'test_db'
[10:17:18][INFO] I/O operations for loading index or check cache took 0.0229236s
[10:17:18][INFO] Loaded HNSW Index with 5981 entries
[10:17:18][INFO] Performing search with Cosine metric (top_k=5)
[10:17:18][INFO] Search complete in 0.0006869s , found 5 results
[10:17:18][INFO] [POST /query] Query successful, returning 5 results
[10:17:18][INFO] Cache HIT for database 'test_db'
[10:17:18][INFO] Cache is valid for database 'test_db'
[10:17:18][INFO] I/O operations for loading index or check cache took 0.0002841s
[10:17:18][INFO] Loaded HNSW Index with 5981 entries
[10:17:18][INFO] Performing search with Cosine metric (top_k=5)
[10:17:18][INFO] Search complete in 0.0003899s , found 5 results
[10:17:18][INFO] [POST /query] Query successful, returning 5 results
```

```shell
cargo nextest run --test query_test --release --no-capture --run-ignored only
   Compiling blaze-db v0.1.0 (C:\codes\blaze-db)
    Finished `release` profile [optimized] target(s) in 22.88s
────────────
 Nextest run ID 6e42b62d-a8d3-45b4-beb6-fe239aea1be6 with nextest profile: default
    Starting 1 test across 1 binary
     Running [ 00:00:00] 0/1: 0 running, 0 passed, 0 skipped
       START (1/1) blaze-db::query_test test_cache_and_bench

running 1 test
Total time without cache: 2.8621101s (Client: 1.7749911s, Server: 1.0871190000000002s)
Total time with cache: 0.0400475s (Client: 0.0378031s, Server: 0.0022443999999999997s)
Improvement factor (Server side): 484.37x
test test_cache_and_bench ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.82s

        PASS [   3.849s] (1/1) blaze-db::query_test test_cache_and_bench
────────────
     Summary [   3.850s] 1 test run: 1 passed, 0 skipped
```

- Almost around 480x faster (I/O) with cache hits on repeated queries on same index. 😭🔥
- Although there is still I/O overhead during cache validation (reading checksum from metadata.json),but it's
  significantly
  reduced. Checkout this file: [Cache Impl](./src/server/service/queries.rs)

## TODO:

- HNSW (Hierarchical Navigable Small World) indexing for improved search performance. `DONE (basic implementation)`
- Fix Chunking for better meaningful text segments. `DONE, but kinda broken for now.`
- Write/insert functionality for adding new vectors to the database.
  `PARTIALLY DONE (Raw vector insertion and query not implemented)`
- Find a way to manage multiple sources during server startup and runtime, load index into memory during it.
  `PARTIALLY DONE (Done using LRU, but there is no startup loading)`
- Fixed and complete error response in API.`IN PROGRESS`
- Refactor the configuration of data and source managing and more data-rich `server_file.toml`. `IN PROGRESS`
- Use a basic In-memory (Hashmap) storage engine for thread-safe read and write operations for configs and server_files.
  `PARTIALLY DONE`.
- Too many clones across the codebase, memory explosion everywhere. `FIXED`
- Too many code duplications across modules. `NEED HELP`
- Similarity calculation caching for faster search queries. `IN PROGRESS`
- Implement LRU for fast query. `DONE`
- Make a storage engine, e.g SSTable or LSMTree based. (Actually, I have no idea how to do that. 😵‍💫) `NEED HELP`
- Bad Indexing, Loading and Memory Explosion issues when inserting large batch of nodes. (HNSW)
  `SIGNIFICANTLY IMPROVED - HNSW insert now uses references`
- Complete Refactor of storage and search modules for new HNSW architecture. `DONE`
- Gotta destroy/refactor the utils module. It's a mess. `DONE`
- Use gRPC/Protobuf for client-server communication?`
- Better API Error handling and logging. `IN PROGRESS`
- API Validation, so that a stupid user/me doesnt corrupted the HNSW index. 😶 `PARTIALLY DONE`
- Complete HTTP API server for remote database access. `Insert endpoint is missing.`
- Better Database and Source Managing `IN PROGRESS`
- Docker env and app config are conflicting `NEED HELP`
- Query filtering and metadata support. `DONE`
- Incremental updates without full reindex. (HNSW) `DONE (Need better indexing matters)`
- Distributed storage and sharding support.
- Move hardcoded Values to separate config files. `API PROVIDER CONFIG LEFT`
- HNSW DEMO and benchmarking. `Need to read Criterion docs`
- Cloud deployment options. `What is cloud thingy?`

## References

- [Curse of Dimensionality](https://en.wikipedia.org/wiki/Curse_of_dimensionality)
- [Little Intro](https://www.pinecone.io/learn/series/faiss/hnsw/)
- [Arvix HNSW Paper](https://arxiv.org/abs/2512.06636)

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️
