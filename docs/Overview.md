## Code Overview

> AI-generated documentation. May contain inaccuracies. Please verify with source code.

### Architecture

```mermaid
flowchart TB
    subgraph Client["Client Layer"]
        CLI[CLI Tools<br/>blzdb]
        Py[Python Bindings<br/>blzdb-py]
    end

    subgraph Server["blzdb-server"]
        HTTP[Axum HTTP<br/>:8000]

        subgraph Controller["Controller"]
            Router[Router]
            State[Global State]
        end

        subgraph Services["Service Layer"]
            QS[Query Service<br/>Search/Filter]
            IS[Indexer Service<br/>Insert/Embed]
            DB[Database Service<br/>Create/Delete]
            BK[Backup Service]
        end

        subgraph Cache["Cache Layer"]
            LRU[LRU Cache<br/>128 indexes]
            Check[Checksum Cache<br/>DashMap]
        end

        subgraph Core["Core Engine"]
            HNSW[HNSW Index]
            SIMD[SIMD Similarity]
            Search[ef-bound Search]
        end

        subgraph Storage["Storage Layer"]
            MM[Memory-Mapped<br/>Files]
            Disk[(Disk)]
        end
    end

    CLI --> HTTP
    Py --> HTTP
    HTTP --> Router
    Router --> State
    Router --> QS
    Router --> IS
    Router --> DB
    Router --> BK
    QS --> LRU
    IS --> LRU
    LRU --> HNSW
    HNSW --> SIMD
    SIMD --> Search
    HNSW --> Search
    HNSW --> MM
    MM <--> Disk
    QS --> Check
    IS --> Check
```

## HNSW Index

### Hierarchical Structure

```mermaid
flowchart TB
    subgraph Graph["HNSW Graph"]
        Entry[Entry Point<br/>Layer N]
        Ln[Layer N<br/>1-2 nodes]
        L2[Layer 2<br/>-1/M nodes]
        L1[Layer 1<br/>-1/M^2 nodes]
        L0[Layer 0<br/>All nodes]
        N1[Node A]
        N2[Node B]
        N3[Node C]
        N4[Node D]
        N5[Node E]
    end

    Entry -->|descend| Ln
    Ln -->|descend| L2
    L2 -->|descend| L1
    L1 -->|descend| L0
    L0 -->|search| N1
    L0 -->|search| N2
    L0 -->|search| N3
    L0 -->|search| N4
    L0 -->|search| N5
    style Ln fill: GREEN
    style L2 fill: GREEN
    style L1 fill: GREEN
    style L0 fill: RED
```

### Insertion Flow

```mermaid
sequenceDiagram
    participant Client
    participant Axum
    participant Ctrl
    participant Indexer
    participant HNSW
    participant Disk
    Client ->> Axum: POST /embed
    Axum ->> Ctrl: route()
    Ctrl ->> Indexer: insert_vector()
    Indexer ->> HNSW: get_random_level()

    rect rgb(0, 0, 0)
    Note over HNSW, Disk: Layer Assignment
        HNSW ->> HNSW: exp(-ln(rand) / M)
    end

    Indexer ->> HNSW: search_layer_greedy(entry, layer)
    loop For each layer from top
        HNSW ->> HNSW: greedy descent
    end

    rect rgb(0, 0, 0)
        Note over HNSW, Disk: Neighbor Connection
        Indexer ->> HNSW: search_layer_knn(ef_construction)
        HNSW ->> HNSW: beam search
        HNSW -->> HNSW: bidirectional edges
        HNSW ->> HNSW: prune_connections()
    end

    HNSW ->> HNSW: assign node_id
    Indexer ->> Disk: persist to mmap
    Indexer -->> Ctrl: result
    Ctrl -->> Axum: response
    Axum -->> Client: JSON
```

### Search Flow

```mermaid
sequenceDiagram
    participant Client
    participant Axum
    participant Ctrl
    participant Query
    participant HNSW
    Client ->> Axum: GET /query
    Axum ->> Ctrl: route()
    Ctrl ->> Query: search()

    rect rgb(0, 0, 0)
        Note over Query, HNSW: Phase 1: Greedy Descent
        Query ->> HNSW: entry = top layer
        loop For each layer
            HNSW ->> HNSW: greedy search
        end
    end

    rect rgb(0, 0, 0)
        Note over Query, HNSW: Phase 2: Beam Search (Layer 0)
        Query ->> HNSW: search_layer_knn(ef_search)
        HNSW ->> HNSW: dual-heap search
    end

    rect rgb(0, 0, 0)
        Note over Query, HNSW: Phase 3: Adaptive Expansion
        opt If too many deleted
            HNSW ->> HNSW: expand ef by 1.5x
        end
    end

    HNSW -->> Query: top-k results
    Query -->> Ctrl: scored results
    Ctrl -->> Axum: response
    Axum -->> Client: JSON
```

### Node & Edge Structure

```mermaid
classDiagram
    class HNSW {
        +Vec~Node~ nodes
        +Option~NodeIndex~ entry_point
        +usize max_layers
        +usize max_neighbors
        +usize ef_construction
        +HashMap~NodeID, NodeIndex~ id_mapper
        +search_layer_knn()
        +insert()
        +delete_node_by_id()
        +reindex()
    }

    class Node {
        +NodeID node_id
        +String metadata
        +Vec~f32~ vector
        +Vec~Vec~NodeIndex~ neighbors
        +usize max_level
        +bool tombstone
    }

    class Candidate {
        +NodeIndex id
        +f32 score
        +Ord impl(max-heap)
    }

    class ScoredResult {
        +NodeIndex id
        +f32 score
        +Ord impl(min-heap)
    }

    HNSW "1" --> "*" Node
    HNSW "1" --> "*" Candidate
    HNSW "1" --> "*" ScoredResult
```

## Concurrency

```mermaid
flowchart LR
    subgraph Runtime["Tokio Async Runtime"]
        HTTP[Async HTTP<br/>axum]
        IO[Async File I/O<br/>tokio::fs]
        Block[CPU Tasks<br/>spawn_blocking]
    end

    subgraph Sync["Synchronization"]
        RW[RWLock<br/>per-database]
        LRU_M[Mutex<br/>LRU Cache]
    end

    subgraph Parallel["Parallel Processing"]
        Rayon[Rayon<br/>par_iter]
        SIMD[SIMD<br/>wide crate]
    end

    subgraph Cache["In-Memory Cache"]
        LRU[LRU<br/>128 entries]
        Dash[DashMap<br/>checksums]
    end

    HTTP --> IO
    HTTP --> Block
    Block --> Rayon
    Rayon --> SIMD
    HTTP --> RW
    RW --> LRU_M
    LRU_M --> LRU
    LRU --> Dash
```

## Data Flow

```mermaid
flowchart LR
    subgraph Ingest["Ingest"]
        API[REST API]
        Parse[Parse JSON]
        Valid[Validate]
    end

    subgraph Process["Process"]
        Cache[Check LRU]
        Index[Update HNSW]
        Hash[Compute Checksum]
    end

    subgraph Persist["Persist"]
        MemMap[Memory Map]
        Sync[sync_all]
        Return[Return OK]
    end

    API --> Parse
    Parse --> Valid
    Valid --> Cache
    Cache --> Index
    Index --> Hash
    Hash --> MemMap
    MemMap --> Sync
    Sync --> Return
```

## Key Parameters

| Parameter           | Default | Description                  |
|---------------------|---------|------------------------------|
| `max_neighbors` (M) | 16      | Max edges per node per layer |
| `ef_construction`   | 200     | Search width during insert   |
| `ef_search`         | 50      | Search width during query    |
| `max_layers`        | 16      | Hierarchy depth              |
| `cache_size`        | 128     | LRU cache entries            |

## Complexity

| Operation | Complexity                 |
|-----------|----------------------------|
| Search    | O(log N × ef)              |
| Insert    | O(log N × ef_construction) |
| Delete    | O(1) mark + O(N) reindex   |
| Memory    | O(N × M × layers)          |

## Storage

- **Format**: Memory-mapped binary files
- **Persistence**: Atomic writes with sync
- **Recovery**: Checksum validation on load
- **Backup**: Snapshot to tarball
