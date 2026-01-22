# ContextDB Design Concepts

This document explains the novel architectural decisions behind ContextDB and why it's different from existing vector databases.

## The Core Innovation: Co-Equal Representations

### Traditional Vector Databases

```
┌─────────────────────────────────┐
│         Vector Database         │
│                                 │
│  PRIMARY:   Vectors             │
│             ├─ HNSW Index       │
│             └─ Fast similarity  │
│                                 │
│  SECONDARY: Metadata (tags)     │
│             ├─ Basic filtering  │
│             └─ Not inspectable  │
└─────────────────────────────────┘
```

**Problem**: Human developers can't easily understand what's stored. Debugging RAG systems becomes "why did it retrieve *that*?"

### ContextDB Architecture

```
┌─────────────────────────────────────────┐
│            ContextDB Entry              │
│                                         │
│  ┌─────────────────┬─────────────────┐ │
│  │    MEANING      │   EXPRESSION    │ │
│  │  (Vector)       │   (Text)        │ │
│  │                 │                 │ │
│  │  Semantic       │   Human         │ │
│  │  Search         │   Inspection    │ │
│  └─────────────────┴─────────────────┘ │
│            ↓              ↓             │
│  ┌─────────────────┬─────────────────┐ │
│  │  Vector Index   │ Text/Metadata   │ │
│  │   (similarity)  │  (structured)   │ │
│  └─────────────────┴─────────────────┘ │
└─────────────────────────────────────────┘
```

**Key insight**: The same data needs TWO equally valid representations:
1. Vector embedding for semantic similarity (LLM access)
2. Human-readable text for inspection (developer/user access)

Neither is "primary" - they're both essential views of the same information.

## Query Modalities

ContextDB supports five orthogonal query modalities that can be freely combined:

### 1. Semantic (Vector Similarity)

```rust
Query::new()
    .with_meaning(query_vector, threshold)
```

**Use case**: LLM retrieval, semantic search  
**Index**: Cosine similarity (HNSW planned)  
**Output**: Ranked by similarity score

### 2. Textual (Expression Matching)

```rust
Query::new()
    .with_expression(ExpressionFilter::Contains("keyword"))
```

**Use case**: Human inspection, debugging  
**Index**: Full-text search on expression field  
**Output**: Filtered by text match

### 3. Structured (Metadata Filtering)

```rust
Query::new()
    .with_context(ContextFilter::PathEquals("/category", value))
```

**Use case**: Filtering by domain-specific attributes  
**Index**: JSON path queries on context field  
**Output**: Filtered by metadata match

### 4. Relational (Graph Traversal)

```rust
Query::new()
    .with_relations(RelationFilter::WithinDistance { 
        from: entry_id, 
        max_hops: 3 
    })
```

**Use case**: Finding related memories, context chains  
**Index**: Graph adjacency in relations table  
**Output**: Entries within N hops

### 5. Temporal (Time-based)

```rust
Query::new()
    .with_temporal(TemporalFilter::CreatedAfter(timestamp))
```

**Use case**: "What did the agent learn today?"  
**Index**: B-tree on created_at/updated_at  
**Output**: Filtered by time range

## Unified Query Composition

The power comes from **composing** these modalities:

```rust
// "Show me dietary memories about onions from last week 
//  that are similar to this query, with high confidence"
Query::new()
    .with_meaning(embedding, Some(0.8))              // Semantic
    .with_expression(ExpressionFilter::Contains("onion"))  // Textual
    .with_context(ContextFilter::And(vec![          // Structured
        PathEquals("/category", json!("dietary")),
        PathEquals("/confidence", json!("high"))
    ]))
    .with_temporal(CreatedAfter(last_week))         // Temporal
    .with_explanation()                             // Explainability
```

Each filter narrows the result set. The query planner executes them in optimal order.

## Explainability as Infrastructure

Traditional databases: "Here are your results"  
ContextDB: "Here are your results AND why they matched"

```rust
pub struct QueryResult {
    entry: Entry,
    similarity_score: Option<f32>,
    explanation: Option<String>,  // ← Built into the result type
}
```

When `.with_explanation()` is enabled, ContextDB tracks which filters matched and generates human-readable explanations:

```
"Semantic similarity: 87.3%, Matched expression filter, 
 Matched context filter, Created within time range"
```

This is **not post-hoc analysis** - it's part of the query execution.

## Schema Flexibility with Query Safety

The `context` field is schema-less JSON:

```json
{
  "category": "dietary",
  "specificity": "item-level",
  "ingredient": "red onion",
  "whatever_you_want": "anything"
}
```

But queries are **type-safe** and **composable**:

```rust
ContextFilter::And(vec![
    PathEquals("/category", json!("dietary")),
    PathContains("/tags", json!("important"))
])
```

This gives you:
- **Flexibility**: Different applications can use different schemas
- **Safety**: Queries are validated at compile time
- **Composability**: Filters can be combined arbitrarily

## Memory as a First-Class Concept

ContextDB is designed around the concept of **memory** rather than generic data:

### Traditional Database Thinking

"Store documents with embeddings, retrieve by similarity"

### ContextDB Thinking

"Capture experiences/facts/preferences in both semantic and linguistic form, enable multi-modal recall with provenance"

This means:
- Every entry knows **when** it was created (temporal provenance)
- Every entry can link to **related** entries (context graph)
- Every entry exists in **semantic space** (meaning) and **linguistic space** (expression)

## Use Case: Dietary Preferences Example

Let's see how the architecture handles a real use case:

### The Data

```rust
Entry {
    meaning: [0.8, 0.1, 0.3],  // Embedding of the semantic content
    expression: "User doesn't like red onions",
    context: {
        "category": "dietary",
        "specificity": "item-level",
        "ingredient": "red onion",
        "sentiment": "dislike"
    },
    relations: [grocery_list_entry_id],
    created_at: "2026-01-15T10:30:00Z"
}
```

### LLM Query (Semantic)

When an LLM needs to check dietary restrictions:

```rust
db.query(&Query::new()
    .with_meaning(conversation_embedding, Some(0.7))
    .with_context(PathEquals("/category", json!("dietary")))
    .with_limit(5)
)
```

This finds semantically relevant dietary preferences, even if the exact words differ.

### Human Query (Inspection)

When a developer debugs "why didn't it order onions?":

```rust
db.query(&Query::new()
    .with_expression(ExpressionFilter::Contains("onion"))
    .with_context(PathEquals("/category", json!("dietary")))
)
```

Returns: "User doesn't like red onions"  
**Readable, debuggable, auditable**

### System Query (Analytics)

When you want to understand patterns:

```rust
db.query(&Query::new()
    .with_context(ContextFilter::And(vec![
        PathEquals("/category", json!("dietary")),
        PathEquals("/specificity", json!("item-level"))
    ]))
    .with_temporal(CreatedAfter(last_month))
)
```

"Show all item-level dietary preferences added recently"

## Why This Matters for LLM Applications

### 1. Transparency

Users can see what the AI "knows" about them without learning vector mathematics.

### 2. Debuggability

Developers can inspect memory stores using familiar text/metadata queries.

### 3. Trust

Explainable retrieval means you can audit why the LLM used specific context.

### 4. Flexibility

Different parts of your system can query the same data differently:
- LLM: semantic similarity
- UI: text search
- Analytics: structured queries
- Audit log: temporal queries

### 5. Correctness

By storing both semantic and linguistic representations, you avoid:
- "The embedding doesn't capture what I meant"
- "I can't see what's actually stored"
- "Why did it retrieve this?"

## Implementation Details

### Current Storage Layer

```
SQLite:
  entries table → id, meaning (blob), expression, context (json), timestamps
  relations table → from_id, to_id (graph edges)
  indexes → created_at, updated_at, expression

In-Memory:
  Vector operations (cosine similarity)
  Query planning and execution
```

### Query Execution Order

1. **Cheapest filters first**: Text/metadata before vector similarity
2. **Semantic ranking**: Sort by similarity if vector query present
3. **Limit early**: Apply limit after ranking but before expensive operations
4. **Explain last**: Generate explanations only for returned results

### Future Optimizations

- **HNSW index**: Replace linear scan with approximate nearest neighbors
- **Materialized views**: Cache common metadata queries
- **Query planner**: Cost-based optimization of filter order
- **Distributed storage**: Shard by semantic clusters

## Philosophical Differences

### ChromaDB/Pinecone Philosophy

"Vector similarity is the primitive, everything else is secondary"

### PostgreSQL+pgvector Philosophy

"Relational data is the primitive, vectors are an extension"

### ContextDB Philosophy

"Semantic meaning and human expression are co-equal primitives, 
 everything else (metadata, time, relations) enriches them"

This philosophical shift leads to different architectural decisions:

- **Storage**: Dual indexes are equally important
- **Query language**: Multi-modal composition is first-class
- **Results**: Always include both similarity and explanation
- **API**: Designed for both programmatic and human access

## Conclusion

ContextDB isn't just "another vector database with better metadata." It's a different way of thinking about semantic data:

**Not**: "Store vectors, add metadata as tags"  
**But**: "Capture meaning in both semantic and linguistic form, enable multi-modal recall"

This architecture enables building LLM applications that are:
- Powerful (semantic search)
- Inspectable (human-readable queries)
- Explainable (built-in provenance)
- Trustworthy (transparent memory)

The core innovation is recognizing that **meaning and expression are co-equal**, not primary and secondary. Once you see that, the rest of the architecture follows naturally.
