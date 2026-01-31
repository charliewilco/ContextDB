# ContextDB Architecture

## When to use this guide

- You want to understand how storage, query planning, and vector search fit together.
- You are evaluating a new backend or storage implementation.
- You are contributing to core internals.

## How to read it

Skim the diagrams first, then jump to the component you care about (storage, query engine, or indexing).

## Overview

ContextDB uses a trait-based storage abstraction inspired by Prisma's approach - the same API works across SQLite, PostgreSQL, MySQL, or any custom backend you implement.

## Core Architecture

```
┌─────────────────────────────────────────┐
│           ContextDB API                  │
│  (Public interface: insert, query, etc) │
└──────────────┬──────────────────────────┘
               │
               ↓
┌──────────────────────────────────────────┐
│       StorageBackend Trait               │
│  (Abstract interface for all backends)   │
└──────────────┬───────────────────────────┘
               │
      ┌────────┴────────┬─────────┬────────┐
      ↓                 ↓         ↓        ↓
┌──────────┐   ┌──────────────┐  ...   ┌─────────┐
│ SQLite   │   │ PostgreSQL   │         │ Custom  │
│ Storage  │   │ Storage      │         │ Backend │
└──────────┘   └──────────────┘         └─────────┘
```

## Repository Layout

```
src/
├── lib.rs            # Public API surface
├── types.rs          # Entry + cosine similarity
├── query.rs          # Query types and filters
└── storage/
    ├── mod.rs        # StorageBackend trait + errors
    └── sqlite.rs     # SQLite implementation
```

Examples live in `examples/`, and the CLI entry point is in `src/bin/main.rs`.

## The StorageBackend Trait

All backends must implement this trait:

```rust
pub trait StorageBackend: Send {
    fn insert(&mut self, entry: &Entry) -> StorageResult<()>;
    fn get(&self, id: Uuid) -> StorageResult<Entry>;
    fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>>;
    fn update(&mut self, entry: &Entry) -> StorageResult<()>;
    fn delete(&mut self, id: Uuid) -> StorageResult<()>;
    fn count(&self) -> StorageResult<usize>;
    fn backend_name(&self) -> &str;
}
```

This trait defines the contract - how ContextDB expects to interact with storage. The implementation details are up to each backend.

## Data Flow (Insert → Query → Result)

1. **Insert**: `ContextDB::insert` forwards to the backend.
2. **Serialize**: SQLite stores `meaning` as a BLOB and `context` as JSON text.
3. **Query**: `ContextDB::query` forwards the unified `Query` to the backend.
4. **Filter/Rank**: Backends apply text/context/temporal filters, then vector similarity.
5. **Explain**: Optional explanations are assembled from matched filters.
6. **Return**: Results include `Entry`, optional similarity score, and optional explanation.

## Why This Design?

### 1. Prisma-Like Developer Experience

```rust
// Development: Use SQLite (zero setup)
let db = ContextDB::in_memory()?;

// Production: Swap to PostgreSQL
let db = ContextDB::with_backend(PostgresStorage::new("postgres://...")?);

// Same code works with both!
db.insert(&entry)?;
let results = db.query(&query)?;
```

### 2. Testing Made Easy

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_my_feature() {
        // Tests use in-memory SQLite - fast, isolated
        let mut db = ContextDB::in_memory().unwrap();
        // ... test logic
    }
}
```

### 3. Platform Flexibility

- **Desktop/Server**: Use SQLite or PostgreSQL
- **Mobile (iOS/Android)**: Use SQLite (embedded)
- **Cloud**: Use PostgreSQL with pgvector
- **Edge**: Use SQLite with replication

### 4. Custom Backends

Need something specific? Implement the trait:

```rust
pub struct RedisStorage {
    client: redis::Client,
}

impl StorageBackend for RedisStorage {
    fn insert(&mut self, entry: &Entry) -> StorageResult<()> {
        // Your Redis-specific implementation
    }
    
    // ... implement other methods
    
    fn backend_name(&self) -> &str {
        "Redis"
    }
}

// Use it!
let db = ContextDB::with_backend(RedisStorage::new("redis://localhost")?);
```

## SQLite Backend Implementation

The default `SqliteStorage` backend:

```
┌─────────────────────────────────────┐
│        SqliteStorage                │
├─────────────────────────────────────┤
│  Tables:                            │
│    • entries (id, meaning, expr,    │
│               context, timestamps)  │
│    • relations (from_id, to_id)     │
│                                     │
│  Indexes:                           │
│    • created_at, updated_at         │
│    • expression (full-text)         │
│    • relations (both directions)    │
│                                     │
│  Vector Storage:                    │
│    • Serialized as BLOB             │
│    • Cosine similarity in-memory    │
│    • (HNSW index planned)           │
└─────────────────────────────────────┘
```

### Serialization Details

- `meaning`: stored as a BLOB of `f32` values.
- `context`: stored as JSON text and queried via `serde_json::Value::pointer`.
- `relations`: stored as rows in `relations(from_id, to_id)` with indexes on both sides.

### Query Execution

1. **Load candidates**: Get all entries (or use indices for initial filtering)
2. **Apply filters in order**:
   - Text/metadata filters first (cheap)
   - Vector similarity last (expensive)
3. **Sort by similarity** if semantic search used
4. **Apply limit**
5. **Generate explanations** if requested

Notes:
- **Context paths** use JSON Pointer syntax (e.g., `/category`, `/tags/0`).
- **Vector similarity** is cosine similarity, computed in-memory.
- **Relations** are stored as an adjacency list (`relations` table).

## Error Model

Storage errors are normalized through `StorageError` so callers get a consistent surface
across backends (I/O failures, serialization errors, and not-found cases).

## Concurrency Model

The current SQLite backend is synchronous and optimized for embedded usage. Parallel query
execution and concurrent writes are future targets (see roadmap in README).

## Future Backends

### PostgreSQL with pgvector

```rust
pub struct PostgresStorage {
    pool: sqlx::PgPool,
}

impl StorageBackend for PostgresStorage {
    fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>> {
        // Use PostgreSQL's native vector operations:
        sqlx::query!(
            "SELECT * FROM entries 
             ORDER BY meaning <-> $1 
             LIMIT 10",
            query_vector
        )
    }
}
```

Benefits:
- Native vector indexing (pgvector extension)
- Better concurrency (vs SQLite)
- Production-grade reliability
- Can scale horizontally

### MySQL Backend

Similar to PostgreSQL, but using MySQL's JSON functions and custom vector handling.

### MongoDB Backend

```rust
pub struct MongoStorage {
    collection: mongodb::Collection<Document>,
}

impl StorageBackend for MongoStorage {
    fn insert(&mut self, entry: &Entry) -> StorageResult<()> {
        // Store as document with both fields and vector
        self.collection.insert_one(doc! {
            "id": entry.id.to_string(),
            "meaning": entry.meaning,
            "expression": entry.expression,
            "context": entry.context,
        })
    }
    
    fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>> {
        // Use MongoDB's $vectorSearch (Atlas Search)
        // or implement cosine similarity in aggregation pipeline
    }
}
```

## Storage Backend Capabilities Matrix

| Feature | SQLite | PostgreSQL | MySQL | MongoDB |
|---------|--------|------------|-------|---------|
| Embedded | ✅ | ❌ | ❌ | ❌ |
| Zero-config | ✅ | ❌ | ❌ | ❌ |
| Production-ready | ⚠️ | ✅ | ✅ | ✅ |
| Native vector ops | ❌ | ✅ (pgvector) | ⚠️ | ⚠️ (Atlas) |
| ACID guarantees | ✅ | ✅ | ✅ | ⚠️ |
| Horizontal scaling | ❌ | ⚠️ | ⚠️ | ✅ |
| JSON queries | ✅ | ✅ | ✅ | ✅ |
| Full-text search | ✅ | ✅ | ✅ | ✅ |

## Design Decisions

### Why Trait-Based?

**Alternative 1: Enum of backends**
```rust
pub enum Storage {
    SQLite(SqliteStorage),
    Postgres(PostgresStorage),
}
```
❌ Closed set - can't add custom backends without modifying library

**Alternative 2: Generic type parameter**
```rust
pub struct ContextDB<S: StorageBackend> {
    storage: S,
}
```
❌ Type complexity leaks to users, can't mix backends at runtime

**Chosen: Trait objects**
```rust
pub struct ContextDB {
    storage: Box<dyn StorageBackend>,
}
```
✅ Open for extension
✅ Simple API
✅ Runtime backend selection
⚠️ Small dynamic dispatch overhead (negligible in practice)

### Why Not `Send + Sync`?

The trait is `Send` but not `Sync` because:
- SQLite's `Connection` is not `Sync` (statement caching uses `RefCell`)
- Most databases aren't designed for concurrent access from multiple threads
- Better to use connection pools at application level
- Keeps the trait simple and inclusive

If you need `Sync`, wrap your backend in a `Mutex`:
```rust
let db = Arc::new(Mutex::new(ContextDB::in_memory()?));
```

### Embedded vs Client-Server

**SQLite (embedded)**:
- Database runs in your process
- No network overhead
- Perfect for desktop/mobile/edge
- Limited concurrency

**PostgreSQL/MySQL (client-server)**:
- Database runs as separate process
- Network overhead (can use Unix sockets locally)
- Better for multi-user applications
- Better concurrency

ContextDB supports both models - choose what fits your deployment.

## Performance Considerations

### Current Implementation (SQLite)

- **Insert**: O(1) - simple SQL insert
- **Get by ID**: O(1) - indexed lookup
- **Vector search**: O(n) - linear scan with cosine similarity
- **Text search**: O(n) - SQLite's FTS if enabled
- **Metadata filtering**: O(n) - JSON path queries

### With HNSW Index (Planned)

- **Vector search**: O(log n) - approximate nearest neighbors
- Trade accuracy for speed (configurable)
- 10-100x faster for large datasets

### With PostgreSQL + pgvector

- **Vector search**: O(log n) - using IVFFlat or HNSW index
- **Concurrent queries**: Much better than SQLite
- **Metadata queries**: Native JSON operators

## Migration Path

Moving from one backend to another:

```rust
// 1. Export from old backend
let old_db = ContextDB::in_memory()?;
let all_entries = old_db.query(&Query::new())?;

// 2. Import to new backend
let mut new_db = ContextDB::with_backend(PostgresStorage::new(...)?);
for result in all_entries {
    new_db.insert(&result.entry)?;
}
```

Future: We'll provide a `migrate` utility to handle this automatically.

## Extending ContextDB

Want to add a new backend? Here's the checklist:

1. **Implement `StorageBackend` trait**
2. **Handle vector serialization** (your choice of format)
3. **Implement query filters** (semantic, text, metadata, temporal)
4. **Add tests** (reuse test suite from SQLite)
5. **Document quirks** (limitations, special features)
6. **Submit PR!** (if you want to share)

Example backends we'd love to see:
- PostgresStorage with pgvector
- MySQLStorage
- MongoStorage (with Atlas Search)
- DuckDBStorage (for analytics)
- TiKVStorage (for distributed deployments)
- SurrealDBStorage
- LanceDBStorage (already Rust-native!)

## Conclusion

The trait-based storage abstraction gives you:
- **Flexibility**: Choose the right backend for your deployment
- **Portability**: Same code works everywhere
- **Testability**: Fast in-memory tests, production backend in prod
- **Extensibility**: Add custom backends without forking

This is the same pattern that makes Prisma so developer-friendly, now available for semantic databases.