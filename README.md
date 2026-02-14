# Blaze-DB

![Blaze-DB Logo](blazedb-icon.png)

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and fast retrieval of
embeddings using HNSW Indexing.

## Current State

- Two binaries: `blaze-server` and `blaze-client`, for server and client operations respectively.
- Uses Ollama API for generating vector embeddings or Bring your own model embeddings using API Server.
- Batch/Chunks processing for embedding generation (Only used in CLI Wrapper).
- Stores/Index embeddings on disk in binary/JSON format.
- Use memory-mapped files for fast loading and concurrent reads, rayon for parallel processing (where possible).
- Index caching (LRU), which gives about 86x faster I/O with reads and writes lockings (thread-safe).
- Implements HNSW (Hierarchical Navigable Small World) graph for approximate nearest neighbor search.
- Basic HTTP API server for remote database access.
- CLI client for local/remote database querying.
- Uses semantic similarity search with multiple distance metrics (Cosine, Euclidean, Dot Product).
- Performance benchmarking suite (<1ms per search on War and Peace dataset, <5ms per search on Amazon Product Dataset).
- Safe Index cache-locks for concurrent access, with cache validation and eviction policies.
- Backup and restore functionality for databases and sources. (Few caveats, need improvement, but ready for happy path)

## Quick Links

- [Docker Hub Image](https://hub.docker.com/r/ronakgh97/blazedb) - `docker pull ronakgh97/blazedb:latest`
- [Pre-indexed Dataset (Google Drive)](https://drive.google.com/file/d/1rnnpMNYzbwkOr9dIetZW83JeF5WCV5cL/view?usp=sharing) -
  278K vectors, ready to use
- [Amazon Products Source Dataset](https://www.kaggle.com/datasets/asaniczka/amazon-products-dataset-2023-1-4m-products) -
  About 1.4M 2023 products, for indexing and testing
- [BlazeDB Service](https://github.com/ronakgh97/blazedb-service) - A Saas layer on top of Blaze-DB for easy hosting and
  management (In development) 🫡

## Usage

### Build from Source (Cargo needed)

```shell
# Initialize dotfiles
blzsrv init

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

### Docker

```shell
# Pull the image from Docker Hub
docker pull ronakgh97/blazedb:latest

# Run the container
docker run -d \
  --name blazedb \
  -p 8080:8080 \
  -env-file .env \
  -v blazedb-config:/home/blazedb/.config/blaze \
  -v blazedb-sources:/home/blazedb/blaze \
  ronakgh97/blazedb:latest
```

- Download Pre-Indexed
  from: [Google Drive Link](https://drive.google.com/file/d/1rnnpMNYzbwkOr9dIetZW83JeF5WCV5cL/view?usp=sharing)
- Checksum (SHA256): **20e7ec6fb00fc7d6988daa0a67349a76898a44dfd46c899cc841f937f0d429b8**
- Extract and copy to Docker volume
- Before copying, create a database `amazon_products_2023` using CLI or API, so that the server recognizes it.

```shell
docker cp amazon_products_2023 blazedb-server:/home/blazedb/blaze/sources/default_src/amazon_products_2023
```

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
- Anyway, 4.5ms is pretty good for 278528 vectors! 👨‍🍳🔥
- Amazon product 2023
  dataset: [Source Link](https://www.kaggle.com/datasets/asaniczka/amazon-products-dataset-2023-1-4m-products?select=amazon_products.csv)

### SEARCH ON WAR AND PEACE DATASET

```shell
blzdb query --search "War and peace" --database def_db --source default_src --top-k 10 

Search querying the database: def_db


Item 1:
Metadata: thousand corpses lay there, but even on the island of St. Helena in the peaceful solitude where he said he intended to devote his leisure to an account of the great deeds he had done, he wrote: The Russian war should have been the most popular war of modern times: it was a war of good sense, for real interests, for the tranquillity and security of all; it was purely pacific and conservative. It was a war for a great cause, the end of uncertainties and the beginning of security. A new horizon and new labors were opening out, full of well-being and prosperity for all. The European system was already founded; all that remained was to organize it. Satisfied on these great points and with tranquility everywhere, I too should have had my Congress and my Holy Alliance. Those ideas were
Score: 0.56

Item 2:
Metadata: when there was a war, like this one, it would be war! And then the determination of the troops would be quite different. Then all these Westphalians and Hessians whom Napoleon is leading would not follow him into Russia, and we should not go to fight in Austria and Prussia without knowing why. War is not courtesy but the most horrible thing in life; and we ought to understand that and not play at war. We ought to accept this terrible necessity sternly and seriously. It all lies in that: get rid of falsehood and let war be war and not a game. As it is now, war is the favorite pastime of the idle and frivolous. The military calling is the most highly honored. “But what is war? What is needed for success in warfare? What are the           
Score: 0.54

Item 3:
Metadata: that: get rid of falsehood and let war be war and not a game. As it is now, war is the favorite pastime of the idle and frivolous. The military calling is the most highly honored. “But what is war? What is needed for success in warfare? What are the habits of the military? The aim of war is murder; the methods of war are spying, treachery, and their encouragement, the ruin of a country’s inhabitants, robbing them or stealing to provision the army, and fraud and falsehood termed military craft. The habits of the military class are the absence of freedom, that is, discipline, idleness, ignorance, cruelty, debauchery, and drunkenness. And in spite of all this it is the highest class, respected by everyone. All the kings, except the Chinese, wear military uniforms, and he who kills most people receives the highest rewards.                                          
Score: 0.53

Item 4:
Metadata: did in 1813—salute according to all the rules of art, and, presenting the hilt of their rapier gracefully and politely, hand it to their magnanimous conqueror, but at the moment of trial, without asking what rules others have adopted in similar cases, simply and easily pick up the first cudgel that comes to hand and strike with it till the feeling of resentment and revenge in their soul yields to a feeling of contempt and compassion. CHAPTER II One of the most obvious and advantageous departures from the so-called laws of war is the action of scattered groups against men pressed together in a mass. Such action always occurs in wars that take on a national character. In such actions, instead of two crowds opposing each other, the men disperse, attack singly, run away when attacked by stronger forces, but again attack when opportunity offers. This was done      
Score: 0.51

Item 5:
Metadata: had thought it was all the same to him whether or not Moscow was taken as Smolénsk had been, was suddenly checked in his speech by an unexpected cramp in his throat. He paced up and down a few times in silence, but his eyes glittered feverishly and his lips quivered as he began speaking. “If there was none of this magnanimity in war, we should go to war only when it was worth while going to certain death, as now. Then there would not be war because Paul Ivánovich had offended Michael Ivánovich. And when there was a war, like this one, it would be war! And then the determination of the troops would be quite different. Then all these Westphalians and Hessians whom Napoleon is leading would not follow him into Russia, and we should not go to fight in Austria and Prussia                                                                                               
Score: 0.50

Item 6:
Metadata: don’t understand what is meant by ‘a skillful commander,’” replied Prince Andrew ironically. “A skillful commander?” replied Pierre. “Why, one who foresees all contingencies... and foresees the adversary’s intentions.” “But that’s impossible,” said Prince Andrew as if it were a matter settled long ago. Pierre looked at him in surprise. “And yet they say that war is like a game of chess?” he remarked. “Yes,” replied Prince Andrew, “but with this little difference, that in chess you may think over each move as long as you please and are not limited for time, and with this difference too, that a knight is always stronger than a pawn, and two pawns are always stronger than one, while in war a battalion is sometimes stronger than a division and sometimes weaker than a company. The relative strength of bodies of troops can                                            
Score: 0.50

Item 7:
Metadata: on never came. In the morning, on an empty stomach, all the old questions appeared as insoluble and terrible as ever, and Pierre hastily picked up a book, and if anyone came to see him he was glad. Sometimes he remembered how he had heard that soldiers in war when entrenched under the enemy’s fire, if they have nothing to do, try hard to find some occupation the more easily to bear the danger. To Pierre all men seemed like those soldiers, seeking refuge from life: some in ambition, some in cards, some in framing laws, some in women, some in toys, some in horses, some in politics, some in sport, some in wine, and some in governmental affairs. “Nothing is trivial, and nothing is important, it’s all the same—only to save oneself from it as best one can,” thought Pierre. “Only not to see it, that dreadful it!”                                                       
Score: 0.49

Item 8:
Metadata: not their own—there were many secondary personages accompanying the army because their principals were there. Among the opinions and voices in this immense, restless, brilliant, and proud sphere, Prince Andrew noticed the following sharply defined subdivisions of tendencies and parties: The first party consisted of Pfuel and his adherents—military theorists who believed in a science of war with immutable laws—laws of oblique movements, outflankings, and so forth. Pfuel and his adherents demanded a retirement into the depths of the country in accordance with precise laws defined by a pseudo-theory of war, and they saw only barbarism, ignorance, or evil intention in every deviation from that theory. To this party belonged the foreign nobles, Wolzogen, Wintzingerode, and others, chiefly Germans. The second party was directly opposed to the first; one extreme, as always happens, was met by representatives of the other. The members of                                                       
Score: 0.49

Item 9:
Metadata: are here?” “I am speaking ze truce,” replied the hussar with a smile. “It’s all about the war,” the count shouted down the table. “You know my son’s going, Márya Dmítrievna? My son is going.” “I have four sons in the army but still I don’t fret. It is all in God’s hands. You may die in your bed or God may spare you in a battle,” replied Márya Dmítrievna’s deep voice, which easily carried the whole length of the table. “That’s true!” Once more the conversations concentrated, the ladies’ at the one end and the men’s at the other. “You won’t ask,” Natásha’s little brother was saying; “I know you won’t ask!” “I will,” replied Natásha. Her face suddenly flushed with reckless and joyous resolution. She half rose, by a glance inviting Pierre, who sat opposite, to listen to what was coming, and turning to her mother:                                                    
Score: 0.48

Item 10:
Metadata: my brother, who announces his speedy arrival at Bald Hills with his wife. This pleasure will be but a brief one, however, for he will leave us again to take part in this unhappy war into which we have been drawn, God knows how or why. Not only where you are—at the heart of affairs and of the world—is the talk all of war, even here amid fieldwork and the calm of nature—which townsfolk consider characteristic of the country—rumors of war are heard and painfully felt. My father talks of nothing but marches and countermarches, things of which I understand nothing; and the day before yesterday during my daily walk through the village I witnessed a heartrending scene.... It was a convoy of conscripts enrolled from our people and starting to join the army. You should have seen the state of                                                                               
Score: 0.48
Time taken (sec): 0.0008125
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

```

- Almost around 480x faster (I/O) with cache hits on repeated queries on same index. 😭🔥
- Although there is still I/O overhead during cache validation (reading checksum from metadata.json),but it's
  significantly
  reduced. Checkout this file: [Cache Impl](./src/server/service/queries.rs)

### Concurrent Benchmarking

```shell
cargo nextest run stress_test_concurrent_writes_different_databases --release --run-ignored only --no-capture
   Compiling blaze-db v0.1.0 (C:\codes\blaze-db)
    Finished `release` profile [optimized] target(s) in 25.59s
------------
 Nextest run ID dd45f926-c746-491b-a9d3-cd57d943f8ad with nextest profile: default
    Starting 1 test across 17 binaries (83 tests skipped)
     Running [ 00:00:00] 0/1: 0 running, 0 passed, 0 skipped
       START (1/1) blaze-db::stress_tests stress_test_concurrent_writes_different_databases

running 1 test
Source created, creating 50 databases...
Databases created, starting concurrent writes...
test stress_test_concurrent_writes_different_databases has been running for over 60 seconds
        SLOW [> 60.000s] (-----) blaze-db::stress_tests stress_test_concurrent_writes_different_databases

 CONCURRENT WRITES TEST RESULTS:
  Databases written: 50
  Vectors per database: 4096
  Successful: 50/50
  Total time: 74.3236711s
  Min write time: 32.3023285s
  Max write time: 68.0414242s
  Avg write time: 47.294857942s
  Expected if sequential: 2364.7428971s
  Speedup: 31.82x
Concurrent writes to different databases work in parallel!
test stress_test_concurrent_writes_different_databases ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 75.01s
```

- Concurrent writes to different databases work in parallel, with significant speedup over sequential writes! 😎🔥
- Each write operation is still quite slow due to HNSW indexing and disk I/O, but at least they don't block each
  other. (Which is a huge improvement over the previous version where all writes were serialized due to global locks. 😭)
- Checkout this file: [Stress Test](tests/stress_tests.rs)

### TODO:

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
- So many Locks and IO everywhere, need a serious fix, no jokes. `Someone help me pls. 😭`
- Server logs are mess, current using my custom macros, need proper monitoring solution. `HELP`
- Cloud deployment options. `What is cloud thingy?`
- Missing Database ops endpoints are missing!!!
- Configurable distance metrics and search parameters. `PARTIALLY DONE`
- Tombstone deletion and Background reindexing for better performance. `IN PROGRESS`
- Add embedded Embeddings model, no more API shit `NEED_HELP`
- Add Backup Mechanism, Background Jobs? API Endpoint? or something else

## References

- [Curse of Dimensionality](https://en.wikipedia.org/wiki/Curse_of_dimensionality)
- [Little Intro](https://www.pinecone.io/learn/series/faiss/hnsw/)
- [Arvix HNSW Paper](https://arxiv.org/abs/2512.06636)

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️
Codebase is getting huge and super hard to maintain it myself

### My Code, My Rules 😼

- **DO NOT USE UNREVIEWED AI CODE**, I can smell the SLOP from miles away, so get away from my trash. 🚫🤖 (Except boring
  testing code, I do use AI for that. 🤧)
- **DO NOT REMOVE THE COMMENTED OUT CODES**, Those are ritual sacrifices to the coding gods 🛐, it ward off evil bugs.
  🪦👹
- **DO NOT REFACTOR SERVER AND CORE MODULES**, Unless there is a less I/O operations, those codes are sacred. 🙏📜
- **DO NOT BRING YOUR 'BEST PRACTICES' BS HERE**, Im very opinionated about coding styles and still learning, so don't
  waste your time. 🛑🧠
- **DO NOT UNDER ANY CIRCUMSTANCES, ADD ANOTHER LOCKS/THREAD SPAWNS/ASYNC AWAIT UNLESS ABSOLUTELY NECESSARY**,
  Those are the reasons why this code is thread-safe AND why the code is slow, and I even dont know HOW!!!!. 🛑🔒

### Support the Project 😌

Indexing high-dimensional datasets and Hosting require decent hardware.
If you'd like to support the hosting costs (VPS/GPU instances for hosting), you can contribute
here : [Support VPS Costs](https://razorpay.me/@ronak8747)