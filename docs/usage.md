# Usage

## Overview
Real-world usage patterns and workflows.

## When to use
- You want practical recipes.
- You are designing memory workflows.

## Examples

## Cookbook

### User preference store

Store durable user preferences that should survive sessions.

```rust
use contextdb::{ContextDB, Entry, Query, ExpressionFilter};
use serde_json::json;

let mut db = ContextDB::new("prefs.db")?;

let entry = Entry::new(
    embedding_from_model("User prefers oat milk"),
    "User prefers oat milk".to_string(),
).with_context(json!({
    "type": "preference",
    "category": "dietary",
    "user_id": "user_123"
}));

db.insert(&entry)?;

// Later: text lookup for quick inspection
let results = db.query(
    &Query::new().with_expression(ExpressionFilter::Contains("oat".to_string()))
)?;
```

### Support ticket triage

Store tickets with context for fast similarity and filtering.

```rust
use contextdb::{ContextDB, Entry, Query, MeaningFilter};
use serde_json::json;

let mut db = ContextDB::new("support.db")?;

let entry = Entry::new(
    embedding_from_model("Login fails on iOS after update"),
    "Login fails on iOS after update".to_string(),
).with_context(json!({
    "type": "ticket",
    "priority": "high",
    "platform": "ios"
}));

db.insert(&entry)?;

let results = db.query(
    &Query::new()
        .with_meaning(embedding_from_model("iOS login problem"), Some(0.7))
        .with_limit(5)
)?;
```

### Planner memory

Store tasks and decisions, then retrieve relevant context for planning.

```rust
use contextdb::{ContextDB, Entry, Query, ContextFilter};
use serde_json::json;

let mut db = ContextDB::new("planner.db")?;

let entry = Entry::new(
    embedding_from_model("Decide sprint scope for Q1"),
    "Decide sprint scope for Q1".to_string(),
).with_context(json!({
    "type": "decision",
    "project": "roadmap",
    "owner": "alex"
}));

db.insert(&entry)?;

let results = db.query(
    &Query::new()
        .with_context(ContextFilter::PathEquals("/project".to_string(), json!("roadmap")))
        .with_limit(10)
)?;
```


No examples yet.

## Pitfalls
- Mixing domains can reduce retrieval quality.

## Next steps
- See `query-language.md` for advanced filtering.
---

Prev: [Embeddings Guide](embeddings.md)
Next: [Architecture](architecture.md)
