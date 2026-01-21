# Blaze-DB

Blaze-DB is a high-performance vector database written in Rust, designed for efficient storage and fast retrieval of
embeddings using HNSW Indexing.

## Current State

- Two binaries: `blaze-server` and `blaze-client`, for server and client operations respectively.
- Uses Ollama API for generating vector embeddings or Bring your own model embeddings using API Server.
- Batch/Chunks processing for embedding generation (Only used in CLI Wrapper).
- Stores/Index embeddings on disk in binary/JSON format.
- Use memory-mapped files for fast loading and concurrent reads, rayon for parallel processing (where possible).
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

- Donwload the Index
  here: [Google Drive Link](https://drive.google.com/file/d/1s17EwtibF9F5UuOnjAVhJPLIOiNM1zgb/view?usp=sharing)
- Extract to `~/.blaze/sources/default_src/amazon_products_2023/`

### Query using CLI Client

```shell
blzdb query --database amazon_products_2023 --source default_src --search "Wireless Bluetooth Headphones with Noise Cancellation" --top_k 10
```

## Benchmarks

### SEARCH ON 2023 AMAZON PRODUCT DATASET (204800 Index)

```shell
Query: Wireless Bluetooth Headphones with Noise Cancellation
Search completed in: 1.7936ms
Top 10 search results for query: 'Wireless Bluetooth Headphones with Noise Cancellation'
1. ID: 63233, Score: 0.93
Title: AmazonCommercial Wireless Noise Cancelling Bluetooth Headphones
2. ID: 67635, Score: 0.89
Title: Bluetooth Active Noise Cancellation Headphone with Build in Microphone
3. ID: 66738, Score: 0.89
Title: Wireless Stereo Noise Cancelling Bluetooth Waterproof Earbuds with Charging Case
4. ID: 66328, Score: 0.89
Title: Wireless Earbuds Bluetooth 5.0 Waterproof Headset Headphones Noise Cancellation
5. ID: 66488, Score: 0.88
Title: Vsonus Noise Cancelling Headphones Wireless Bluetooth, Over Ear Bluetooth Headphones Noise Canceling with Microphone for Adults, 40H Playtime, Deep Bass Sound, Folding, Comfortable Earpads
6. ID: 64430, Score: 0.88
Title: Vsonus Noise Cancelling Headphones Wireless Bluetooth, Over Ear Bluetooth Headphones Noise Canceling with Microphone for Adults, 30H Playtime, Deep Bass Sound, Folding, Comfortable Earpads
7. ID: 60323, Score: 0.87
Title: Picun Active Noise Cancelling Headphones with ENC, 100 Hours Playing Time Wireless Bluetooth Headphones Over Ear Headphones for Travel, Home, Office
8. ID: 63070, Score: 0.87
Title: Wireless Headphones with Microphone, HD Stereo Sound & Noise Isolating Bluetooth Headset with Mute Button, Comfortable 25Hrs Playtime Hands Free On Ear Headphones for Cell Phone Calls Music Work
9. ID: 66418, Score: 0.87
Title: Sony Noise Cancelling Headphones WH1000XM2: Over Ear Wireless Bluetooth Headphones with Microphone - Hi Res Audio and Active Sound Cancellation - Black (2017 model)
10. ID: 61815, Score: 0.87
Title: Active Noise Cancelling Headphones,Wireless Noise Cancelling Headphone, Microphone 40 Hours Playtime Wireless Bluetooth Headphones 3D Low Bass Tone Fast Charge for Cellphone/Work/Gym/Travel (Blue)
```

- Had a classic dev moment here, was getting 28ms, until I realized that I was running in debug mode. 😶
- Anyways, 1.79ms is pretty decent for 204800 vectors! 👨‍🍳🔥
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

## TODO:

- HNSW (Hierarchical Navigable Small World) indexing for improved search performance. `DONE (basic implementation)`
- Fix Chunking for better meaningful text segments. `DONE, but kinda broken for now.`
- Write/insert functionality for adding new vectors to the database. `PARTIALLY DONE (Bad very Indexing performance)`
- Find a way to manage multiple sources during server startup and runtime, load index into memory during it.
  `IN PROGRESS`
- Similarity calculation caching for faster search queries. `IN PROGRESS`
- Implement LRU for fast query. `IN PROGRESS`
- Make a storage engine, e.g SSTable or LSMTree based. (Actually, I have no idea how to do that. 😵‍💫) `NEED HELP`
- Bad Indexing, Loading and Memory Explosion issues when inserting large batch of nodes. (HNSW) `NEED HElP`
- Complete Refactor of storage and search modules for new HNSW architecture. `DONE`
- Gotta destroy/refactor the utils module. It's a mess. `DONE`
- Use gRPC/Protobuf for client-server communication?`
- Better API Error handling and logging. `IN PROGRESS`
- API Validation, so that a stupid user doesnt corrupted the HNSW index. 😶 `IN PROGRESS`
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
- [Little HNSW Intro](https://www.pinecone.io/learn/series/faiss/hnsw/)

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests. 🤧🏳️
