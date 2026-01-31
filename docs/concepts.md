# ContextDB Design Concepts

## Overview
Design ideas and the mental model behind ContextDB.

## When to use
- You are deciding fit or explaining ContextDB to others.

## Examples

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
    .with_expression(ExpressionFilter::Contains("keyword".to_string()))
```

**Use case**: Human inspection, debugging  
**Index**: Full-text search on expression field  
**Output**: Filtered by text match

### 3. Structured (Metadata Filtering)

```rust
Query::new()
    .with_context(ContextFilter::PathEquals("/category".to_string(), value))
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
    .with_expression(ExpressionFilter::Contains("onion".to_string()))  // Textual
    .with_context(ContextFilter::And(vec![          // Structured
        PathEquals("/category".to_string(), json!("dietary")),
        PathEquals("/confidence".to_string(), json!("high"))
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

## The Accountability Loop

ContextDB is designed for systems where humans and LLMs share memory:

1. **Capture**: Store the raw expression a human would recognize.
2. **Embed**: Store semantic meaning for machine retrieval.
3. **Retrieve**: Use semantic or structured queries depending on the actor.
4. **Explain**: Provide evidence for why something was retrieved.
5. **Correct**: Humans can update or delete entries when the memory is wrong.

This loop turns "black box memory" into something inspectable and maintainable.

## Memory Lifecycle

Entries are not immutable facts. They evolve:

- **Create**: Insert a new entry with meaning + expression.
- **Update**: Revise expression or context as reality changes.
- **Relate**: Connect entries to form context chains.
- **Retire**: Delete or archive stale entries when they no longer apply.

This is a better match for long-lived assistants than append-only vector stores.

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
    PathEquals("/category".to_string(), json!("dietary")),
    PathContains("/tags".to_string(), json!("important"))
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

## Relations as Context Graphs

Relations let you model chains like "preference → reason → source":

```
Preference entry ──→ Conversation note ──→ External source
```

Because relations are first-class, you can ask:
- "What is directly related to this preference?"
- "What memories are within two hops of this incident?"
- "What entries have no relations (orphaned facts)?"

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
    .with_context(PathEquals("/category".to_string(), json!("dietary")))
    .with_limit(5)
)
```

This finds semantically relevant dietary preferences, even if the exact words differ.

### Human Query (Inspection)

When a developer debugs "why didn't it order onions?":

```rust
db.query(&Query::new()
    .with_expression(ExpressionFilter::Contains("onion".to_string()))
    .with_context(PathEquals("/category".to_string(), json!("dietary")))
)
```

Returns: "User doesn't like red onions"  
**Readable, debuggable, auditable**

### System Query (Analytics)

When you want to understand patterns:

```rust
db.query(&Query::new()
    .with_context(ContextFilter::And(vec![
        PathEquals("/category".to_string(), json!("dietary")),
        PathEquals("/specificity".to_string(), json!("item-level"))
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

# Core Concepts

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

## Pitfalls
- Treating meaning as primary and expression as optional undermines the model.

## Next steps
- See `usage.md` for applied patterns.
---

Prev: [CLI Guide](cli.md)
Next: [Query Language](query-language.md)
