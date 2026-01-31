# API Reference

## Overview
Compact Rust API reference for ContextDB.

## When to use
- You want method signatures.
- You are integrating into Rust code.

## Examples

This is a quick reference to the public Rust API. It is not an exhaustive spec. For workflows and patterns, see `quickstart.md`, `usage.md`, and `query-language.md`.

## Core types

### `ContextDB`

```rust
impl ContextDB {
    pub fn in_memory() -> StorageResult<Self>;
    pub fn new<P: AsRef<Path>>(path: P) -> StorageResult<Self>;
    pub fn with_backend<B: StorageBackend>(backend: B) -> Self;

    pub fn backend_name(&self) -> &str;

    pub fn insert(&mut self, entry: &Entry) -> StorageResult<()>;
    pub fn get(&self, id: Uuid) -> StorageResult<Entry>;
    pub fn update(&mut self, entry: &Entry) -> StorageResult<()>;
    pub fn delete(&mut self, id: Uuid) -> StorageResult<()>;

    pub fn count(&self) -> StorageResult<usize>;
    pub fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>>;
}
```

### `Entry`

```rust
impl Entry {
    pub fn new(meaning: Vec<f32>, expression: String) -> Self;
    pub fn with_context(self, context: serde_json::Value) -> Self;
    pub fn add_relation(self, entry_id: Uuid) -> Self;
}
```

Key fields:
- `id: Uuid`
- `meaning: Vec<f32>`
- `expression: String`
- `context: serde_json::Value`
- `created_at: DateTime<Utc>`
- `updated_at: DateTime<Utc>`
- `relations: Vec<Uuid>`

### `Query`

```rust
impl Query {
    pub fn new() -> Self;

    pub fn with_meaning(self, vector: Vec<f32>, threshold: Option<f32>) -> Self;
    pub fn with_expression(self, filter: ExpressionFilter) -> Self;
    pub fn with_context(self, filter: ContextFilter) -> Self;
    pub fn with_relations(self, filter: RelationFilter) -> Self;
    pub fn with_temporal(self, filter: TemporalFilter) -> Self;

    pub fn with_limit(self, limit: usize) -> Self;
    pub fn with_explanation(self) -> Self;
}
```

### Filters

```rust
pub enum ExpressionFilter {
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    Exact(String),
}

pub enum ContextFilter {
    PathExists(String),
    PathEquals(String, serde_json::Value),
    PathContains(String, serde_json::Value),
    And(Vec<ContextFilter>),
    Or(Vec<ContextFilter>),
}

pub enum RelationFilter {
    From(Uuid),
    To(Uuid),
    Both(Uuid, Uuid),
}

pub enum TemporalFilter {
    CreatedBefore(DateTime<Utc>),
    CreatedAfter(DateTime<Utc>),
    CreatedBetween(DateTime<Utc>, DateTime<Utc>),
    UpdatedBefore(DateTime<Utc>),
    UpdatedAfter(DateTime<Utc>),
    UpdatedBetween(DateTime<Utc>, DateTime<Utc>),
}
```

### `QueryResult`

```rust
pub struct QueryResult {
    pub entry: Entry,
    pub similarity_score: Option<f32>,
    pub explanation: Option<String>,
}
```

## Common patterns

### Insert and fetch

```rust
let mut db = ContextDB::new("memories.db")?;
let entry = Entry::new(vec![0.1, 0.2], "Hello".to_string());

db.insert(&entry)?;
let stored = db.get(entry.id)?;
```

### Semantic search with threshold

```rust
let results = db.query(
    &Query::new()
        .with_meaning(vec![0.2, 0.1], Some(0.8))
        .with_limit(5)
)?;
```

### Text search

```rust
let results = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("onion".to_string()))
)?;
```

### Context filter

```rust
use serde_json::json;

let results = db.query(
    &Query::new()
        .with_context(ContextFilter::PathEquals("/category".to_string(), json!("dietary")))
)?;
```

## Feature flags

- `cli`: enables the `contextdb` CLI binary.
- `ffi`: enables the C FFI layer.

## Pitfalls
- Handle `StorageResult<T>` errors explicitly.
- Keep embedding dimensions consistent within a database.

## Next steps
- See `query-language.md` for richer filters.
- See `usage.md` for real workflows.
---

Prev: [Architecture](architecture.md)
Next: [Performance](performance.md)
