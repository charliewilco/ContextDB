# Query Language

## Overview
How to compose queries across meaning, text, context, relations, and time.

## When to use
- You need hybrid queries.
- You want to understand filter composition.

## Examples

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

## Pitfalls
- Mixed embedding dimensions reduce similarity quality.
- Overly strict filters can return no results.

## Next steps
- See `api-reference.md` for types and signatures.
- See `usage.md` for real patterns.
---

| Prev | Next |
| --- | --- |
| [Core Concepts](concepts.md) | [Glossary](glossary.md) |
