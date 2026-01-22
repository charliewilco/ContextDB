# ContextDB

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**A novel database for semantic data with human accountability**

ContextDB is an embedded database designed for LLM-powered applications that need to maintain context and memory. Unlike traditional vector databases where human-readable metadata is an afterthought, ContextDB treats **meaning** (vector embeddings) and **expression** (human-readable text) as co-equal, first-class representations of the same data.

```rust
// Create database - no server, no service, just use it
let mut db = ContextDB::in_memory()?;

// Store semantic meaning + human expression together
let entry = Entry::new(
    vec![0.1, 0.2, 0.3],  // your embedding
    "User doesn't like red onions".to_string()
);
db.insert(&entry)?;

// LLMs query by similarity, humans query by text
let results = db.query(&Query::new()
    .with_meaning(query_vector, Some(0.8))
    .with_expression(ExpressionFilter::Contains("onion"))
)?;
```

---

## Table of Contents

- [Why ContextDB?](#why-contextdb)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Query Language](#query-language)
- [Use Cases](#use-cases)
- [Architecture](#architecture)
- [API Reference](#api-reference)
- [Performance](#performance)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [FAQ](#faq)

---

## Why ContextDB?

### The Problem

Building LLM applications requires storing and retrieving context/memory. Existing solutions fall short:

| Solution | Problem |
|----------|---------|
| **Vector DBs** (Pinecone, ChromaDB) | Can't easily inspect what's stored. Debugging = "why did it retrieve *that*?" |
| **Traditional DBs** (Postgres+pgvector) | Vector search bolted on, no unified query model |
| **Dual Storage** | Sync headaches, consistency issues, 2x complexity |

### The ContextDB Solution

**One database, two equal interfaces:**

```
┌────────────────────────────────────┐
│        ContextDB Entry             │
│                                    │
│  Meaning (vector)  ←→  Expression │
│  For LLMs              For Humans  │
│                                    │
│  Semantic search   ←→  Text query │
│  Graph relations   ←→  Metadata   │
│  Similarity rank   ←→  Inspection │
└────────────────────────────────────┘
```

**Key Benefits:**

- ✅ **Embedded** - No server, no service, just `use contextdb`
- ✅ **Transparent** - Humans can inspect what AI "knows"
- ✅ **Explainable** - Built-in query explanations
- ✅ **Flexible** - Schema-less JSON metadata
- ✅ **Unified** - One query API for semantic + structured + text + temporal
- ✅ **Type-safe** - Compile-time query validation

---

## Installation

### CLI Tool

The easiest way to get started is with the `contextdb` CLI.

#### Homebrew (macOS/Linux)

```bash
brew tap yourusername/contextdb
brew install contextdb
```

#### Curl (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/yourusername/contextdb/main/scripts/install.sh | bash
```

#### Cargo (any platform)

```bash
cargo install contextdb
```

#### From Source

```bash
git clone https://github.com/yourusername/contextdb.git
cd contextdb
cargo install --path .
```

### As a Rust Library

Add to your `Cargo.toml`:

```toml
[dependencies]
contextdb = "0.1"

# Or from git
contextdb = { git = "https://github.com/yourusername/contextdb" }
```

### Verify Installation

```bash
contextdb --version
contextdb --help
```

---

## CLI Usage

The CLI provides a complete interface for working with ContextDB.

### Quick Start

```bash
# Create a new database
contextdb init mydata.db

# Check database stats
contextdb stats mydata.db

# Search for entries
contextdb search mydata.db "search term"

# List all entries
contextdb list mydata.db

# Show entry details
contextdb show mydata.db <entry-id>

# Interactive mode
contextdb repl mydata.db
```

### Commands

| Command | Description |
|---------|-------------|
| `init <path>` | Create a new database |
| `stats <path>` | Show database statistics |
| `search <path> <query>` | Search entries by text |
| `list <path>` | List all entries |
| `show <path> <id>` | Show entry details |
| `recent <path>` | Show recently added entries |
| `export <path>` | Export database to JSON |
| `import <path> <file>` | Import entries from JSON |
| `delete <path> <id>` | Delete an entry |
| `repl <path>` | Interactive REPL mode |

### Examples

```bash
# Search with limit
contextdb search mydata.db "coffee" --limit 5

# Export to file
contextdb export mydata.db --output backup.json

# List in JSON format
contextdb list mydata.db --format json

# Delete with confirmation
contextdb delete mydata.db abc123

# Delete without confirmation
contextdb delete mydata.db abc123 --force
```

### REPL Mode

Interactive mode for exploring your database:

```
$ contextdb repl mydata.db
ContextDB REPL
Database: mydata.db (42 entries)
Type 'help' for commands, 'quit' to exit

contextdb> search coffee
abc12345 | User prefers cold brew coffee
def67890 | Coffee shop recommendation: Blue Bottle

contextdb> show abc12345
ID: abc12345-...
Expression: User prefers cold brew coffee
Context: {"category": "dietary", "confidence": 0.9}
Created: 2026-01-15 10:30:00

contextdb> recent 5
...

contextdb> quit
Goodbye!
```

---

## Quick Start

### Basic Usage

```rust
use contextdb::{ContextDB, Entry, Query, ExpressionFilter};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory database (no persistence)
    let mut db = ContextDB::in_memory()?;
    
    // Or create file-backed database
    // let mut db = ContextDB::new("memories.db")?;
    
    // Insert entry with semantic + linguistic representation
    let entry = Entry::new(
        vec![0.8, 0.1, 0.3],  // Embedding from your model
        "User doesn't like red onions".to_string(),
    ).with_context(json!({
        "category": "dietary",
        "specificity": "item-level"
    }));
    
    db.insert(&entry)?;
    
    // Query by semantic similarity (LLM use case)
    let semantic_results = db.query(
        &Query::new()
            .with_meaning(vec![0.75, 0.15, 0.25], Some(0.7))
            .with_limit(5)
    )?;
    
    for result in semantic_results {
        println!("Found: {} (similarity: {:.1}%)", 
            result.entry.expression,
            result.similarity_score.unwrap() * 100.0
        );
    }
    
    // Query by text (human inspection)
    let text_results = db.query(
        &Query::new()
            .with_expression(ExpressionFilter::Contains("onion"))
    )?;
    
    for result in text_results {
        println!("Found: {}", result.entry.expression);
    }
    
    Ok(())
}
```

### Run the Demo

```bash
cargo run --example demo
```

This shows:
- Dietary preferences with varying granularity
- Semantic search (LLM query pattern)
- Text search (human query pattern)
- Metadata filtering
- Hybrid queries combining multiple filters
- Explainable results

---

## Core Concepts

### Entry: The Fundamental Unit

Every piece of data in ContextDB is an `Entry`:

```rust
pub struct Entry {
    pub id: Uuid,                    // Unique identifier
    pub meaning: Vec<f32>,           // Semantic representation (embedding)
    pub expression: String,          // Human-readable text
    pub context: serde_json::Value,  // Flexible metadata
    pub relations: Vec<Uuid>,        // Links to other entries
    pub created_at: DateTime<Utc>,   // When created
    pub updated_at: DateTime<Utc>,   // When modified
}
```

**Philosophy**: `meaning` and `expression` are co-equal representations of the same information, not primary/secondary.

### Creating Entries

```rust
// Minimal entry
let entry = Entry::new(
    embedding_vector,
    "User prefers TypeScript over JavaScript".to_string()
);

// With metadata
let entry = Entry::new(embedding, expression)
    .with_context(json!({
        "category": "work",
        "language": "typescript",
        "confidence": 0.95
    }));

// With relations
let entry = Entry::new(embedding, expression)
    .add_relation(related_entry_id);
```

### Query: Multi-Modal Retrieval

Queries can combine five orthogonal modalities:

```rust
Query::new()
    .with_meaning(vector, threshold)      // Semantic similarity
    .with_expression(ExpressionFilter)    // Text matching
    .with_context(ContextFilter)          // Metadata filtering
    .with_relations(RelationFilter)       // Graph traversal
    .with_temporal(TemporalFilter)        // Time-based
    .with_limit(n)                        // Limit results
    .with_explanation()                   // Include explanations
```

---

## Query Language

### 1. Semantic Queries (Vector Similarity)

```rust
// Find entries similar to a query vector
Query::new()
    .with_meaning(
        vec![0.1, 0.2, 0.3],  // Query vector
        Some(0.8)              // Minimum similarity threshold (0.0-1.0)
    )
    .with_limit(10)
```

**Use case**: LLM retrieval, semantic search

### 2. Text Queries (Expression Matching)

```rust
// Contains substring (case-insensitive)
Query::new()
    .with_expression(ExpressionFilter::Contains("onion"))

// Exact match
Query::new()
    .with_expression(ExpressionFilter::Equals("exact text"))

// Starts with prefix
Query::new()
    .with_expression(ExpressionFilter::StartsWith("User"))

// Regex match
Query::new()
    .with_expression(ExpressionFilter::Matches(r"\d{3}-\d{4}"))
```

**Use case**: Human inspection, debugging, full-text search

### 3. Metadata Queries (Context Filtering)

```rust
// Path equals value
Query::new()
    .with_context(ContextFilter::PathEquals(
        "/category".to_string(), 
        json!("dietary")
    ))

// Path exists
Query::new()
    .with_context(ContextFilter::PathExists("/tags".to_string()))

// Path contains (for arrays)
Query::new()
    .with_context(ContextFilter::PathContains(
        "/tags".to_string(),
        json!("important")
    ))

// Combine with AND
Query::new()
    .with_context(ContextFilter::And(vec![
        ContextFilter::PathEquals("/category", json!("work")),
        ContextFilter::PathEquals("/priority", json!("high"))
    ]))

// Combine with OR
Query::new()
    .with_context(ContextFilter::Or(vec![
        ContextFilter::PathEquals("/status", json!("urgent")),
        ContextFilter::PathEquals("/status", json!("critical"))
    ]))
```

**Use case**: Domain-specific filtering, structured queries

### 4. Graph Queries (Relationship Traversal)

```rust
// Directly related entries
Query::new()
    .with_relations(RelationFilter::DirectlyRelatedTo(entry_id))

// Within N hops
Query::new()
    .with_relations(RelationFilter::WithinDistance {
        from: entry_id,
        max_hops: 3
    })

// Has any relations
Query::new()
    .with_relations(RelationFilter::HasRelations)

// Has no relations
Query::new()
    .with_relations(RelationFilter::NoRelations)
```

**Use case**: Context chains, related memories, graph exploration

### 5. Temporal Queries (Time-Based)

```rust
use chrono::prelude::*;

// Created after timestamp
Query::new()
    .with_temporal(TemporalFilter::CreatedAfter(
        Utc.ymd(2026, 1, 1).and_hms(0, 0, 0)
    ))

// Created before timestamp
Query::new()
    .with_temporal(TemporalFilter::CreatedBefore(timestamp))

// Created between timestamps
Query::new()
    .with_temporal(TemporalFilter::CreatedBetween(start, end))

// Updated after timestamp
Query::new()
    .with_temporal(TemporalFilter::UpdatedAfter(timestamp))
```

**Use case**: "What changed today?", audit logs, temporal analysis

### 6. Hybrid Queries (Combining Modalities)

The real power comes from combining filters:

```rust
// "Show me dietary memories about onions from last week, 
//  similar to this query, with high confidence"
Query::new()
    .with_meaning(embedding, Some(0.8))
    .with_expression(ExpressionFilter::Contains("onion"))
    .with_context(ContextFilter::And(vec![
        ContextFilter::PathEquals("/category", json!("dietary")),
        ContextFilter::PathEquals("/confidence", json!("high"))
    ]))
    .with_temporal(TemporalFilter::CreatedAfter(last_week))
    .with_limit(10)
    .with_explanation()
```

### 7. Explainable Results

```rust
let results = db.query(
    &Query::new()
        .with_meaning(vector, Some(0.8))
        .with_expression(ExpressionFilter::Contains("typescript"))
        .with_explanation()  // ← Enable explanations
)?;

for result in results {
    println!("Entry: {}", result.entry.expression);
    
    if let Some(score) = result.similarity_score {
        println!("Similarity: {:.1}%", score * 100.0);
    }
    
    if let Some(explanation) = result.explanation {
        println!("Why: {}", explanation);
        // Output: "Semantic similarity: 87.3%, Matched expression filter"
    }
}
```

---

## Use Cases

### 1. LLM Memory Systems

**Problem**: LLMs need to maintain user context across sessions

**Solution**: Store preferences, facts, and conversation history in ContextDB

```rust
// Store user preference
let pref = Entry::new(
    embedding_from_llm("dietary preference about gluten"),
    "User is gluten-free".to_string()
).with_context(json!({
    "type": "dietary",
    "learned_from": "conversation_id_123",
    "confidence": 0.95
}));

db.insert(&pref)?;

// LLM retrieves relevant context before responding
let context = db.query(
    &Query::new()
        .with_meaning(conversation_embedding, Some(0.7))
        .with_limit(5)
)?;

// Feed context to LLM prompt
```

### 2. RAG Systems with Explainability

**Problem**: Can't debug why specific documents were retrieved

**Solution**: Store documents with embeddings, query with explanations

```rust
// Index document
let doc = Entry::new(
    document_embedding,
    document_text.clone()
).with_context(json!({
    "source": "internal_docs",
    "author": "engineering",
    "last_updated": "2026-01-15"
}));

db.insert(&doc)?;

// Query with explanation
let results = db.query(
    &Query::new()
        .with_meaning(query_embedding, Some(0.7))
        .with_context(ContextFilter::PathEquals("/source", json!("internal_docs")))
        .with_explanation()
)?;

// Show user why documents matched
for result in results {
    println!("Retrieved: {}", result.entry.expression);
    println!("Because: {}", result.explanation.unwrap());
}
```

### 3. Personal AI Assistants

**Problem**: Users want transparency into what AI "knows" about them

**Solution**: Provide human-readable memory inspection

```rust
// AI stores learned facts
let fact = Entry::new(
    embedding,
    "User prefers morning meetings".to_string()
).with_context(json!({
    "category": "scheduling",
    "learned_at": "2026-01-15T09:30:00Z"
}));

db.insert(&fact)?;

// User inspects their memories
let my_memories = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("prefer"))
        .with_temporal(TemporalFilter::CreatedAfter(last_month))
)?;

// Display in UI: "Here's what I know about you..."
```

### 4. Multi-Agent Systems

**Problem**: Different agents need different views of shared memory

**Solution**: Each agent queries differently

```rust
// Agent 1 (Planner): Semantic retrieval
let planning_context = db.query(
    &Query::new()
        .with_meaning(task_embedding, Some(0.8))
        .with_context(ContextFilter::PathEquals("/priority", json!("high")))
)?;

// Agent 2 (Executor): Structured query
let tasks = db.query(
    &Query::new()
        .with_context(ContextFilter::And(vec![
            ContextFilter::PathEquals("/status", json!("pending")),
            ContextFilter::PathEquals("/assigned_to", json!("executor"))
        ]))
)?;

// Human supervisor: Natural language inspection
let overview = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("urgent"))
        .with_temporal(TemporalFilter::CreatedAfter(today))
)?;
```

### 5. Debugging AI Applications

**Problem**: "Why did the AI do that?"

**Solution**: Inspect what memories/context it retrieved

```rust
// AI made unexpected decision
// Developer investigates:

// What did it retrieve?
let retrieved = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("decision keyword"))
        .with_explanation()
)?;

// What memories exist about this topic?
let related = db.query(
    &Query::new()
        .with_meaning(topic_embedding, Some(0.6))
        .with_context(ContextFilter::PathExists("/decision_factor"))
)?;

// When were these memories created?
let timeline = db.query(
    &Query::new()
        .with_context(ContextFilter::PathEquals("/topic", json!("the topic")))
        .with_temporal(TemporalFilter::CreatedBetween(start, end))
)?;
```

---

## Architecture

### Storage Layer

```
┌─────────────────────────────────────────┐
│            ContextDB                    │
│                                         │
│  ┌──────────────────────────────────┐  │
│  │         SQLite Backend           │  │
│  │                                  │  │
│  │  • entries table (id, meaning,  │  │
│  │    expression, context, times)  │  │
│  │  • relations table (graph)      │  │
│  │  • indexes (time, text, etc.)   │  │
│  └──────────────────────────────────┘  │
│                                         │
│  ┌──────────────────────────────────┐  │
│  │      Vector Operations           │  │
│  │                                  │  │
│  │  • Cosine similarity             │  │
│  │  • Linear scan (HNSW planned)   │  │
│  └──────────────────────────────────┘  │
│                                         │
│  ┌──────────────────────────────────┐  │
│  │       Query Engine               │  │
│  │                                  │  │
│  │  • Multi-modal composition       │  │
│  │  • Filter optimization           │  │
│  │  • Explanation generation        │  │
│  └──────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

### Data Model

```rust
// Storage: SQLite tables
CREATE TABLE entries (
    id TEXT PRIMARY KEY,
    meaning BLOB NOT NULL,        -- Serialized Vec<f32>
    expression TEXT NOT NULL,     -- Human-readable text
    context TEXT NOT NULL,        -- JSON metadata
    created_at TEXT NOT NULL,     -- ISO 8601 timestamp
    updated_at TEXT NOT NULL
);

CREATE TABLE relations (
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id)
);

// Indexes for fast queries
CREATE INDEX idx_entries_created_at ON entries(created_at);
CREATE INDEX idx_entries_updated_at ON entries(updated_at);
CREATE INDEX idx_entries_expression ON entries(expression);
```

### Query Execution

1. **Parse query** - Decompose into filter components
2. **Optimize** - Order filters by selectivity (cheap → expensive)
3. **Execute filters**:
   - Text/metadata filters (fast, use indexes)
   - Vector similarity (slower, linear scan currently)
4. **Rank results** - Sort by similarity if semantic query
5. **Generate explanations** - Track which filters matched
6. **Return results** - Include entries, scores, explanations

---

## API Reference

### ContextDB

```rust
impl ContextDB {
    // Create in-memory database
    pub fn in_memory() -> StorageResult<Self>

    // Create file-backed database
    pub fn new<P: AsRef<Path>>(path: P) -> StorageResult<Self>

    // Create with custom backend
    pub fn with_backend<B: StorageBackend + 'static>(backend: B) -> Self

    // Insert entry
    pub fn insert(&mut self, entry: &Entry) -> StorageResult<()>

    // Get entry by ID
    pub fn get(&self, id: Uuid) -> StorageResult<Entry>

    // Execute query
    pub fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>>

    // Update an existing entry
    pub fn update(&mut self, entry: &Entry) -> StorageResult<()>

    // Delete an entry by ID
    pub fn delete(&mut self, id: Uuid) -> StorageResult<()>

    // Count total entries
    pub fn count(&self) -> StorageResult<usize>

    // Get the storage backend name
    pub fn backend_name(&self) -> &str
}
```

### Entry

```rust
impl Entry {
    // Create new entry
    pub fn new(meaning: Vec<f32>, expression: String) -> Self
    
    // Add metadata
    pub fn with_context(self, context: serde_json::Value) -> Self
    
    // Add relation
    pub fn add_relation(self, entry_id: Uuid) -> Self
    
    // Calculate similarity with another entry
    pub fn similarity(&self, other: &Entry) -> f32
}
```

### Query

```rust
impl Query {
    // Create new query
    pub fn new() -> Self
    
    // Add semantic search
    pub fn with_meaning(self, vector: Vec<f32>, threshold: Option<f32>) -> Self
    
    // Add text search
    pub fn with_expression(self, filter: ExpressionFilter) -> Self
    
    // Add metadata filter
    pub fn with_context(self, filter: ContextFilter) -> Self
    
    // Add temporal filter
    pub fn with_temporal(self, filter: TemporalFilter) -> Self
    
    // Limit results
    pub fn with_limit(self, limit: usize) -> Self
    
    // Enable explanations
    pub fn with_explanation(self) -> Self
}
```

### QueryResult

```rust
pub struct QueryResult {
    pub entry: Entry,                    // The matching entry
    pub similarity_score: Option<f32>,   // Score if semantic query
    pub explanation: Option<String>,     // Why it matched (if requested)
}
```

---

## Performance

### Current Implementation

- **Storage**: SQLite with bundled library
- **Vector search**: Linear scan with cosine similarity
- **Suitable for**: < 100,000 entries
- **Query latency**: 
  - Text/metadata queries: < 1ms (indexed)
  - Semantic queries: O(n) - linear in entry count
  - Hybrid queries: Sum of components

### Optimization Roadmap

- [ ] HNSW index for approximate nearest neighbor search
- [ ] Batch insertion API
- [ ] Query result caching
- [ ] Parallel vector comparison
- [ ] Memory-mapped vectors
- [ ] Persistent vector index

### Benchmarks

```bash
cargo bench  # (Coming soon)
```

---

## Roadmap

### v0.2.0 (Next)

- [ ] HNSW index integration
- [ ] Batch operations
- [ ] Transaction support
- [x] Update/delete operations
- [x] Comprehensive tests (154 unit tests)

### v0.3.0

- [ ] Query language parser (text-based queries)
- [ ] GraphQL API option
- [ ] HTTP server mode
- [ ] Streaming results
- [ ] Compression for vectors

### v0.4.0

- [ ] Distributed storage
- [ ] Replication
- [ ] Sharding by semantic clusters
- [ ] Advanced graph queries
- [ ] Real-time updates

### v1.0.0

- [ ] Production-ready performance
- [ ] Comprehensive documentation
- [ ] Client libraries (Python, JavaScript, Swift)
- [ ] Migration tools
- [ ] Monitoring and observability

---

## Contributing

Contributions welcome! This is an exploratory project.

### Areas for Contribution

- **Performance**: HNSW integration, query optimization
- **Features**: GraphQL API, query parser, client libraries
- **Documentation**: More examples, tutorials
- **Testing**: Benchmarks, stress tests, edge cases
- **Integrations**: LangChain, LlamaIndex, etc.

### Development Setup

```bash
# Clone repo
git clone https://github.com/yourusername/contextdb.git
cd contextdb

# Run tests (154 unit tests)
cargo test

# Run tests with output
cargo test -- --nocapture

# Run example
cargo run --example demo

# Check formatting
cargo fmt --check

# Run lints
cargo clippy
```

### Test Coverage

The project includes comprehensive test coverage:

- **types.rs**: 28 tests (cosine similarity, Entry creation, relations, serialization)
- **query.rs**: 32 tests (Query builder, all filter types, serialization)
- **storage/sqlite.rs**: 56 tests (CRUD operations, filter matching, queries)
- **storage/mod.rs**: 10 tests (error handling, trait bounds)
- **lib.rs**: 28 integration tests (full API coverage)

### Submit Issues

Found a bug? Have a feature request? [Open an issue](https://github.com/yourusername/contextdb/issues)

### Pull Requests

1. Fork the repo
2. Create feature branch (`git checkout -b feature/amazing`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing`)
5. Open Pull Request

---

## FAQ

### Is this production-ready?

**No.** This is a v0.1.0 prototype demonstrating a novel architecture. Use it for:
- ✅ Prototyping LLM applications
- ✅ Exploring memory/context patterns
- ✅ Understanding what's possible

Don't use it for:
- ❌ Production deployments
- ❌ Large-scale systems (>100k entries)
- ❌ Mission-critical applications

### How does this compare to Pinecone/ChromaDB?

**Different philosophy:**

- **Pinecone/ChromaDB**: "Vector similarity is primary, metadata is secondary"
- **ContextDB**: "Semantic meaning and human expression are co-equal"

This leads to different trade-offs:
- ✅ Better for human inspection and debugging
- ✅ Better for applications needing transparency
- ✅ Better for multi-modal queries
- ⚠️ Currently slower for pure vector search (HNSW planned)
- ⚠️ Less mature, fewer integrations

### Can I use this without Rust?

**Eventually, yes.** Roadmap includes:

1. **HTTP API** (any language)
2. **Client libraries** (Python, JavaScript, Swift)
3. **FFI bindings** (C, Swift, etc.)

Currently, it's Rust-only.

### How do I generate embeddings?

ContextDB **doesn't generate embeddings** - you provide them. Use:

- OpenAI's embedding API
- sentence-transformers (Python)
- Local models (BERT, etc.)
- Your own embedding models

```rust
// You generate embeddings elsewhere
let embedding = your_embedding_model.encode("text");

// Then store in ContextDB
let entry = Entry::new(embedding, "text".to_string());
db.insert(&entry)?;
```

### What's the vector dimension limit?

No hard limit, but:
- Typical: 384 (MiniLM), 768 (BERT), 1536 (OpenAI)
- Large: 4096+ (less common)
- **All entries should use the same dimension**

### Can I update entries?

**Yes!** Use the `update` method:

```rust
// Get existing entry
let mut entry = db.get(entry_id)?;

// Modify it
entry.expression = "Updated text".to_string();
entry.context = json!({"updated": true});
entry.updated_at = Utc::now();

// Save changes
db.update(&entry)?;
```

### How do I delete entries?

**Yes!** Use the `delete` method:

```rust
// Delete by ID
db.delete(entry_id)?;

// Returns StorageError::NotFound if entry doesn't exist
```

Note: Deleting an entry also removes its relations from the graph.

### Is there a size limit?

**SQLite limits apply:**
- Database file: ~281 TB max
- Entry count: Practical limit ~100k (without HNSW)
- Vector size: No limit (stored as blob)

### Can I use this in production?

**Not recommended yet.** Wait for:
- v0.3.0+ (more features, better performance)
- HNSW index (faster queries)
- Production hardening
- Comprehensive testing

---

## License

MIT License - see [LICENSE](LICENSE) file

---

## Acknowledgments

- **EdgeDB** - Inspiration for elegant query language
- **ChromaDB** - Simplicity for LLM use cases
- **SQLite** - Reliable embedded storage
- **Rust community** - Excellent ecosystem

---

## Citation

If you use ContextDB in research:

```bibtex
@software{contextdb2026,
  author = {Your Name},
  title = {ContextDB: A Novel Database for Semantic Data with Human Accountability},
  year = {2026},
  url = {https://github.com/yourusername/contextdb}
}
```

---

## Contact

- **Issues**: [GitHub Issues](https://github.com/yourusername/contextdb/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/contextdb/discussions)
- **Email**: your.email@example.com

---

**Built with ❤️ for transparent AI systems**
