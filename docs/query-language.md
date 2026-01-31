# Query Language

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
    .with_expression(ExpressionFilter::Contains("onion".to_string()))

// Exact match
Query::new()
    .with_expression(ExpressionFilter::Equals("exact text".to_string()))

// Starts with prefix
Query::new()
    .with_expression(ExpressionFilter::StartsWith("User".to_string()))

// Regex match
Query::new()
    .with_expression(ExpressionFilter::Matches(r"\d{3}-\d{4}".to_string()))
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
        ContextFilter::PathEquals("/category".to_string(), json!("work")),
        ContextFilter::PathEquals("/priority".to_string(), json!("high"))
    ]))

// Combine with OR
Query::new()
    .with_context(ContextFilter::Or(vec![
        ContextFilter::PathEquals("/status".to_string(), json!("urgent")),
        ContextFilter::PathEquals("/status".to_string(), json!("critical"))
    ]))
```

**Use case**: Domain-specific filtering, structured queries

**Paths** use JSON Pointer syntax (e.g., `/category`, `/tags/0`).

### 4. Graph Queries (Relationship Traversal)

```rust
// Relations are set via the query struct (no builder method yet)
let query = Query {
    relations: Some(RelationFilter::DirectlyRelatedTo(entry_id)),
    ..Query::new()
};

let query = Query {
    relations: Some(RelationFilter::WithinDistance {
        from: entry_id,
        max_hops: 3,
    }),
    ..Query::new()
};

let query = Query {
    relations: Some(RelationFilter::HasRelations),
    ..Query::new()
};

let query = Query {
    relations: Some(RelationFilter::NoRelations),
    ..Query::new()
};
```

**Use case**: Context chains, related memories, graph exploration

### 5. Temporal Queries (Time-Based)

```rust
use chrono::{TimeZone, Utc};

// Created after timestamp
Query::new()
    .with_temporal(TemporalFilter::CreatedAfter(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
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
    .with_expression(ExpressionFilter::Contains("onion".to_string()))
    .with_context(ContextFilter::And(vec![
        ContextFilter::PathEquals("/category".to_string(), json!("dietary")),
        ContextFilter::PathEquals("/confidence".to_string(), json!("high"))
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
        .with_expression(ExpressionFilter::Contains("typescript".to_string()))
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

## Advanced Query Construction

Some filters (like relation queries or `top_k`) are set directly on the struct:

```rust
use contextdb::{MeaningFilter, Query, RelationFilter};

let query = Query {
    meaning: Some(MeaningFilter {
        vector: vec![0.1, 0.2, 0.3],
        threshold: Some(0.75),
        top_k: Some(5),
    }),
    relations: Some(RelationFilter::HasRelations),
    ..Query::new()
};
```

You can freely combine these with `with_expression`, `with_context`, and `with_temporal`.
