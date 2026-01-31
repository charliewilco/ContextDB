# API Reference

This is a compact reference for the public Rust API. For narrative guides, see `quickstart.md`, `usage.md`, and `query-language.md`.

## When to use this guide

- You want method signatures and types in one place.
- You are wiring ContextDB into an existing Rust codebase.
- You need to check return types and error behavior.

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

Relations and advanced vector options are set on the struct:

```rust
let query = Query {
    relations: Some(RelationFilter::DirectlyRelatedTo(entry_id)),
    ..Query::new()
};

let query = Query {
    meaning: Some(MeaningFilter {
        vector,
        threshold: Some(0.8),
        top_k: Some(5),
    }),
    ..Query::new()
};
```

### Filters

```rust
pub struct MeaningFilter {
    pub vector: Vec<f32>,
    pub threshold: Option<f32>,
    pub top_k: Option<usize>,
}

pub enum RelationFilter {
    DirectlyRelatedTo(Uuid),
    WithinDistance { from: Uuid, max_hops: usize },
    HasRelations,
    NoRelations,
}

pub enum TemporalFilter {
    CreatedAfter(DateTime<Utc>),
    CreatedBefore(DateTime<Utc>),
    CreatedBetween(DateTime<Utc>, DateTime<Utc>),
    UpdatedAfter(DateTime<Utc>),
    UpdatedBefore(DateTime<Utc>),
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

## Notes

- The CLI is behind the `cli` feature flag.
- The C FFI is behind the `ffi` feature flag.
- Most methods return `StorageResult<T>`; handle errors explicitly.
