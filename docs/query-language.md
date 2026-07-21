# Query Language

Queries compose semantic, expression, context, directed-relation, and temporal filters. All supplied filters must match.

```rust
use contextdb::{ExpressionFilter, Query, QueryOrder, RelationFilter};

let query = Query::new()
	.with_meaning(vec![0.1, 0.2, 0.3], Some(0.75))
	.with_top_k(20)
	.with_expression(ExpressionFilter::FullText("rust database".into()))
	.with_relations(RelationFilter::HasRelations)
	.with_hybrid_weights(0.7, 0.3)
	.with_limit(10)
	.with_explanation();
```

`FullText` uses SQLite FTS5 syntax and provides a normalized BM25 `lexical_score`. When meaning is also present, the default semantic/lexical weights are equal; `with_hybrid_weights` overrides them and adds `combined_score`.

Without semantic or lexical ranking, results use `QueryOrder` and UUID tie-breaking. Offset pagination is available with `with_offset`. For stable continuation, pass the last result UUID to `with_cursor_after`; the cursor must be present in the ordered matching set and cannot be combined with offset.

Validation rejects empty/non-finite/mixed-dimension vectors, semantic thresholds outside `0..=1`, invalid temporal ranges, zero `top_k`, and invalid hybrid weights. Regex patterns are compiled and evaluated as regexes rather than literal SQL substrings.

Context paths use JSON Pointer, such as `/category` or `/tags/0`. Relations are directed: filters follow stored outgoing edges.

---

| Prev | Next |
| --- | --- |
| [Core Concepts](concepts.md) | [Glossary](glossary.md) |
