# FAQ

Quick answers to common questions. If you do not find what you need, open an issue or start a discussion.

## When to use this guide

- You want a quick yes/no or rule-of-thumb.
- You are deciding if ContextDB fits your project.

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

### How do I query relations?

Relations are part of the query struct:

```rust
let query = Query {
    relations: Some(RelationFilter::WithinDistance {
        from: entry_id,
        max_hops: 2,
    }),
    ..Query::new()
};

let results = db.query(&query)?;
```

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
