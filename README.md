# Blaze-DB

![Blaze-DB Logo](blazedb-icon.png)

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and fast retrieval of
embeddings using HNSW Indexing.

## Current State

- Two binaries: `blaze-server` and `blaze-client`, for server and client wrapper operations respectively.
- Batch/Chunks processing for embedding generation (Only used in CLI Wrapper).
- Stores/Index embeddings on disk in binary/JSON format.
- Use memory-mapped files for fast loading and concurrent reads, rayon for parallel processing (where possible).
- Index caching (LRU), which gives about 86x faster I/O with reads and writes lockings (thread-safe).
- Implements HNSW (Hierarchical Navigable Small World) graph for approximate nearest neighbor search.
- Uses semantic similarity search with multiple distance metrics (Cosine, Euclidean, Dot Product).
- Performance benchmarking suite (<1ms per search on War and Peace dataset, <5ms per search on Amazon Product Dataset).
- Safe Index cache-locks for concurrent access, with cache validation and eviction policies.
- Crash safe write operations with temporary files and atomic renaming. (COW style 🐄)
- Backup and restore functionality for databases and sources. (Few caveats, need improvement, but ready for happy path)
- Bench ARG `blzdb bench` to CLI for quick benchmarking on Concurrent Write/Reads workload with Isolated environment.
- Lazy deletion with tombstone nodes and background reindexing job for low disk space usage. (Needs improvement, but
  works for now)

## Quick Links

- [Docker Hub Image](https://hub.docker.com/r/ronakgh97/blazedb) - `docker pull ronakgh97/blazedb:latest`
- [Pre-indexed Dataset (Google Drive)](https://drive.google.com/file/d/1rnnpMNYzbwkOr9dIetZW83JeF5WCV5cL/view?usp=sharing) -
  350K vectors, ready to use
- [Amazon Products Source Dataset](https://www.kaggle.com/datasets/asaniczka/amazon-products-dataset-2023-1-4m-products) -
  About 1.4M 2023 products, for indexing and testing
- [BlazeDB Service](https://github.com/ronakgh97/blazedb-service) - A Saas layer on top of Blaze-DB for easy hosting and
  management (In development) 🫡 `I dont like Saas honestly`

## Usage

### Build from Source (Cargo needed)

```shell
# Initialize dotfiles
blzdb init

blzdb serve         
[14:11:48][INFO] Starting the Server...
[14:11:48][INFO] "Provider (Model: text-embedding-qwen3-embedding-0.6b, Url: http://local..., Key: loca...)"
[14:11:48][INFO] Source: default_src is valid
[14:11:48][INFO] Starting server with 1 valid source(s)
[14:11:48][INFO] Server is running on http://0.0.0.0:8080
[14:11:48][INFO] Using Sources: ["default_src"]
[14:11:48][INFO] Backup scheduler enabled: false
[14:11:48][INFO] Index cache capacity: 128
[14:11:48][INFO] Server started
```

- Download the Index
  here: [Google Drive Link](https://drive.google.com/file/d/13tSMijMC3C7xV1lbV_EiWk46hfNKNuf7/view?usp=sharing)
- Checksum (Sha256): **2621bb7a65f3da9f38d50ebda2fd619b49332a6ade02e51f5d4c1c7b118e2763**
- Extract to `~/.blaze/sources/default_src/amazon_products_2023/`

### Docker

```shell
# Pull the image from Docker Hub
docker pull ronakgh97/blazedb:latest

# Run the container (use --backup flag to start with backup scheduler enabled)
docker run -d \
  --name blazedb \
  -p 8080:8080 \
  -env-file .env \
  -v blazedb-config:/home/blazedb/.config/blaze \
  -v blazedb-sources:/home/blazedb/blaze \
  -v blazedb-backups:/home/blazedb/backups \
  ronakgh97/blazedb:latest
```

- Download Pre-Indexed
  from: [Google Drive Link](https://drive.google.com/file/d/13tSMijMC3C7xV1lbV_EiWk46hfNKNuf7/view?usp=sharing)
- Checksum (SHA256): **2621bb7a65f3da9f38d50ebda2fd619b49332a6ade02e51f5d4c1c7b118e2763**
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

### SEARCH ON 2023 AMAZON PRODUCT DATASET (350k Index)

```shell
Query: Gaming RTX 4060 Laptop with 165Hz Display
Search completed in: 3.5474ms
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

Brute search results (for comparison)
Brute search completed in: 47.3905ms
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

Speedup: 13.36x
```

- Had a classic moment here, was getting >50ms, until I realized that I was running in debug mode. 😶
- Anyway, <5ms is pretty good for 350_000 vectors! 😭👨‍🍳🔥
- That Accuracy is pure luck or maybe the dataset/HNSW params is just that good, but I will take it. 😎
- Amazon product 2023
  dataset: [Source Link](https://www.kaggle.com/datasets/asaniczka/amazon-products-dataset-2023-1-4m-products?select=amazon_products.csv)

### SEARCH ON WAR AND PEACE DATASET

```shell
blzcli query --search "War and peace" --database def_db --source default_src --top-k 10 

Search querying the database: def_db


Item 1:
Metadata: thousand corpses lay there, but even on the island of St. Helena in the peaceful solitude where he said he intended to devote his leisure to an account of the great deeds he had done, he wrote: The Russian war should have been the most popular war of modern times: it was a war of good sense, for real interests, for the tranquillity and security of all; it was purely pacific and conservative. It was a war for a great cause, the end of uncertainties and the beginning of security. A new horizon and new labors were opening out, full of well-being and prosperity for all. The European system was already founded; all that remained was to organize it. Satisfied on these great points and with tranquility everywhere, I too should have had my Congress and my Holy Alliance. Those ideas were                 
Score: 0.56

Item 2:
Metadata: Author: graf Leo Tolstoy Translator: Aylmer Maude Louise Maude Release date: April 1, 2001 [eBook #2600] Most recently updated: June 14, 2022 Language: English Credits: An Anonymous Volunteer and David Widger *** START OF THE PROJECT GUTENBERG EBOOK WAR AND PEACE *** WAR AND PEACE By Leo Tolstoy/Tolstoi CHAPTER I “Well, Prince, so Genoa and Lucca are now just family estates of the Buonapartes. But I warn you, if you don’t tell me that this means war, if you still try to defend the infamies and horrors perpetrated by that Antichrist—I really believe he is Antichrist—I will have nothing more to do with you and you are no longer my friend, no longer my ‘faithful slave,’ as you call yourself! But how do you do? I see I have frightened you—sit down and tell me all the news.”                          
Score: 0.55

Item 3:
Metadata: when there was a war, like this one, it would be war! And then the determination of the troops would be quite different. Then all these Westphalians and Hessians whom Napoleon is leading would not follow him into Russia, and we should not go to fight in Austria and Prussia without knowing why. War is not courtesy but the most horrible thing in life; and we ought to understand that and not play at war. We ought to accept this terrible necessity sternly and seriously. It all lies in that: get rid of falsehood and let war be war and not a game. As it is now, war is the favorite pastime of the idle and frivolous. The military calling is the most highly honored. “But what is war? What is needed for success in warfare? What are the                                                                       
Score: 0.54

Item 4:
Metadata: that: get rid of falsehood and let war be war and not a game. As it is now, war is the favorite pastime of the idle and frivolous. The military calling is the most highly honored. “But what is war? What is needed for success in warfare? What are the habits of the military? The aim of war is murder; the methods of war are spying, treachery, and their encouragement, the ruin of a country’s inhabitants, robbing them or stealing to provision the army, and fraud and falsehood termed military craft. The habits of the military class are the absence of freedom, that is, discipline, idleness, ignorance, cruelty, debauchery, and drunkenness. And in spite of all this it is the highest class, respected by everyone. All the kings, except the Chinese, wear military uniforms, and he who kills most people receives the highest rewards.                                                                                                                
Score: 0.53

Item 5:
Metadata: of states and nations in their conflicts with one another is expressed in wars, and that as a direct result of greater or less success in war the political strength of states and nations increases or decreases. Strange as may be the historical account of how some king or emperor, having quarreled with another, collects an army, fights his enemy’s army, gains a victory by killing three, five, or ten thousand men, and subjugates a kingdom and an entire nation of several millions, all the facts of history (as far as we know it) confirm the truth of the statement that the greater or lesser success of one army against another is the cause, or at least an essential indication, of an increase or decrease in the strength of the nation—even though it is unintelligible why the defeat of an army—a hundredth part of a nation—should oblige                                                                                                        
Score: 0.52

Item 6:
Metadata: fatherland, and it happened in the greatest of all known wars. The period of the campaign of 1812 from the battle of Borodinó to the expulsion of the French proved that the winning of a battle does not produce a conquest and is not even an invariable indication of conquest; it proved that the force which decides the fate of peoples lies not in the conquerors, nor even in armies and battles, but in something else. The French historians, describing the condition of the French army before it left Moscow, affirm that all was in order in the Grand Army, except the cavalry, the artillery, and the transport—there was no forage for the horses or the cattle. That was a misfortune no one could remedy, for the peasants of the district burned their hay rather than let the French have it.                    
Score: 0.51

Item 7:
Metadata: did in 1813—salute according to all the rules of art, and, presenting the hilt of their rapier gracefully and politely, hand it to their magnanimous conqueror, but at the moment of trial, without asking what rules others have adopted in similar cases, simply and easily pick up the first cudgel that comes to hand and strike with it till the feeling of resentment and revenge in their soul yields to a feeling of contempt and compassion. CHAPTER II One of the most obvious and advantageous departures from the so-called laws of war is the action of scattered groups against men pressed together in a mass. Such action always occurs in wars that take on a national character. In such actions, instead of two crowds opposing each other, the men disperse, attack singly, run away when attacked by stronger forces, but again attack when opportunity offers. This was done                                                                            
Score: 0.51

Item 8:
Metadata: of the statement that the greater or lesser success of one army against another is the cause, or at least an essential indication, of an increase or decrease in the strength of the nation—even though it is unintelligible why the defeat of an army—a hundredth part of a nation—should oblige that whole nation to submit. An army gains a victory, and at once the rights of the conquering nation have increased to the detriment of the defeated. An army has suffered defeat, and at once a people loses its rights in proportion to the severity of the reverse, and if its army suffers a complete defeat the nation is quite subjugated. So according to history it has been found from the most ancient times, and so it is to our own day. All Napoleon’s wars serve to confirm this                                     
Score: 0.50

Item 9:
Metadata: had thought it was all the same to him whether or not Moscow was taken as Smolénsk had been, was suddenly checked in his speech by an unexpected cramp in his throat. He paced up and down a few times in silence, but his eyes glittered feverishly and his lips quivered as he began speaking. “If there was none of this magnanimity in war, we should go to war only when it was worth while going to certain death, as now. Then there would not be war because Paul Ivánovich had offended Michael Ivánovich. And when there was a war, like this one, it would be war! And then the determination of the troops would be quite different. Then all these Westphalians and Hessians whom Napoleon is leading would not follow him into Russia, and we should not go to fight in Austria and Prussia                             
Score: 0.50

Item 10:
Metadata: don’t understand what is meant by ‘a skillful commander,’” replied Prince Andrew ironically. “A skillful commander?” replied Pierre. “Why, one who foresees all contingencies... and foresees the adversary’s intentions.” “But that’s impossible,” said Prince Andrew as if it were a matter settled long ago. Pierre looked at him in surprise. “And yet they say that war is like a game of chess?” he remarked. “Yes,” replied Prince Andrew, “but with this little difference, that in chess you may think over each move as long as you please and are not limited for time, and with this difference too, that a knight is always stronger than a pawn, and two pawns are always stronger than one, while in war a battalion is sometimes stronger than a division and sometimes weaker than a company. The relative strength of bodies of troops can                                                                                                                  
Score: 0.50
Time taken (sec): 0.0009919
```

### HHSW DEMO WITH Benchmarks (RANDOM 50,000 VECTORS)

```shell
Building HNSW graph with 50000 nodes...
Indexing completed in 263.5830083s

HNSW Layer Statistics:
  Layer 0: 50000 nodes (100.00%)
  Layer 1: 3096 nodes (6.19%)
  Layer 2: 181 nodes (0.36%)
  Layer 3: 15 nodes (0.03%)
  Layer 4: 1 nodes (0.00%)
  Entry point: node 21918 at layer 4
Querying vector: [0.21357012, -0.8388252, -0.25997758]...
Search completed in: 0.0011535s

Top 5 nearest neighbors:
  1. Node 46879 - similarity: 0.09, Metadata: what a nice vector
  2. Node 23231 - similarity: 0.08, Metadata: what a nice vector
  3. Node 25242 - similarity: 0.08, Metadata: what a nice vector
  4. Node 44287 - similarity: 0.07, Metadata: what a nice vector
  5. Node 41951 - similarity: 0.07, Metadata: what a nice vector

Brute-force search completed in: 0.0074883s

Top 5 nearest neighbors (Brute-force):
  1. Node 22258 - similarity: 0.13, Metadata: what a nice vector
  2. Node 35588 - similarity: 0.12, Metadata: what a nice vector
  3. Node 35335 - similarity: 0.11, Metadata: what a nice vector
  4. Node 14154 - similarity: 0.11, Metadata: what a nice vector
  5. Node 21531 - similarity: 0.11, Metadata: what a nice vector

Speedup over brute-force: 6.49x
```

- Curse of dimensionality applies here, due to bad random vectors
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
 Nextest run ID 6e42b62d-a8d3-45b4-beb6-fe239aea1be6 with nextest profile: default
    Starting 1 test across 1 binary
     Running [ 00:00:00] 0/1: 0 running, 0 passed, 0 skipped
       START (1/1) blaze-db::query_test test_cache_and_bench

running 1 test
Total time without cache: 2.8621101s (Client: 1.7749911s, Server: 1.0871190000000002s)
Total time with cache: 0.0400475s (Client: 0.0378031s, Server: 0.0022443999999999997s)
Improvement factor (Server side): 484.37x
test test_cache_and_bench ... ok
```

- Although there is still I/O overhead during cache validation (reading checksum from metadata.json),but it's
  significantly
  reduced. Checkout this file: [Cache Impl](./src/server/service/queries.rs)

### Concurrent Benchmarking

```shell
cargo nextest run stress_test_concurrent_writes_different_databases --release --run-ignored only --no-capture
   Compiling blaze-db v0.1.0 (C:\codes\blaze-db)
    Finished `release` profile [optimized] target(s) in 25.59s
 Nextest run ID dd45f926-c746-491b-a9d3-cd57d943f8ad with nextest profile: default
    Starting 1 test across 17 binaries (83 tests skipped)
     Running [ 00:00:00] 0/1: 0 running, 0 passed, 0 skipped
       START (1/1) blaze-db::stress_tests stress_test_concurrent_writes_different_databases

running 1 test
Source created, creating 50 databases...
Databases created, starting concurrent writes...
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
```

- Concurrent writes to different databases work in parallel, with significant speedup over sequential writes! 😎🔥
- Each write operation is still quite slow due to HNSW indexing and disk I/O, but at least they don't block each
  other. (Which is a huge improvement over the previous version where all writes were serialized due to global locks. 😭)
- Checkout the Test: [Stress Test](tests/stress_tests.rs)

### TODO:

- HNSW (Hierarchical Navigable Small World) indexing for improved search performance. `DONE (basic implementation)`
- Fix Chunking for better meaningful text segments (Client-side btw). `DONE, but kinda broken for now.`
- Write/insert functionality for adding new vectors to the database.
  `DONE, few checks lefts`
- Find a way to manage multiple sources during server startup and runtime, load index into memory during it.
  `PARTIALLY DONE (Done using LRU, but there is no startup loading)`
- Fixed and complete error response in API.`IN PROGRESS`
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
- Complete HTTP API server for remote database access. `Few deletion and list are missinng.`
- Better Database and Source Managing `IN PROGRESS`
- Docker env and app config are conflicting `DONE`
- Query filtering and metadata support. `DONE`
- Incremental updates without full reindex. (HNSW) `DONE (Need better indexing)`
- Move hardcoded Values to separate config files. `DONE`
- So many Locks and IO everywhere, need a serious fix, no jokes. `Someone help me pls. 😭`
- Server logs are mess, current using my custom macros, need proper monitoring solution.
  `ENV LOG DONE, MONITORING NEED HELP`
- Missing Database ops endpoints are missing!!! `IN PROGRESS`
- Configurable distance metrics and search parameters.
  `PARTIALLY DONE, HNSW/search config is still hardcoded, but API supports it.`
- Tombstone deletion and Background reindexing for better performance. `DONE, but so many room for improvement`
- Add embedded Embeddings model, no more API shit `NEED_HELP`
- Add Backup Mechanism, Background Jobs? API Endpoint? or something else `DONE, but have rare edge case, but still`
- Playground demo in [Landing Page](https://blazedb.online) using Qdrant used datasets `IN_PROGESS`

## References

- [Curse of Dimensionality](https://en.wikipedia.org/wiki/Curse_of_dimensionality)
- [Little Intro](https://www.pinecone.io/learn/series/faiss/hnsw/)
- [Arvix HNSW Paper](https://arxiv.org/abs/2512.06636)

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️
Codebase is getting huge and hard to maintain it myself

### My Rules 😼

- **DO NOT USE AI CODE PLEASE**, I can smell the SLOP from miles away, so get away from my trash. 🚫🤖 (Except boring
  testing code, I do use AI for that. 🤧)
- **DO NOT REMOVE THE COMMENTED OUT CODES**, Those are ritual sacrifices to the coding gods 🛐, it ward off evil bugs and
  deadlocks.
  🪦👹
- **DO NOT REFACTOR SERVER AND CORE MODULES**, Unless there is a less I/O operations, those codes are sacred. 🙏📜
- **DO NOT BRING YOUR 'BEST PRACTICES' BS HERE**, Im very opinionated about coding styles and still learning, so don't
  waste your time. 🛑🧠
- **DO NOT UNDER ANY CIRCUMSTANCES, ADD ANOTHER LOCKS/THREAD SPAWNS/ASYNC AWAIT UNLESS ABSOLUTELY NECESSARY**,
  Those are the reasons why this code is thread-safe AND why the code is slow, and I even dont know HOW!!!!. 🛑🔒