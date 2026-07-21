# API Reference

## Core types

`Entry` contains a UUID, finite non-empty `Vec<f32>` meaning, expression, JSON context, timestamps, and directed outgoing relation UUIDs. All entries in a database must use the same vector dimension.

```rust
let entry = Entry::new(vec![0.1, 0.2, 0.3], "Example".into())
	.with_context(serde_json::json!({"source": "user"}))
	.add_relation(other_id);
```

`Query` can combine these filters:

- `MeaningFilter { vector, threshold, top_k }`
- `ExpressionFilter::{Equals, Contains, StartsWith, Matches, FullText}`
- `ContextFilter::{PathExists, PathEquals, PathContains, And, Or}`
- `RelationFilter::{DirectlyRelatedTo, WithinDistance, HasRelations, NoRelations}`
- `TemporalFilter::{CreatedAfter, CreatedBefore, CreatedBetween, UpdatedAfter, UpdatedBefore}`

Builder methods are `with_meaning`, `with_top_k`, `with_expression`, `with_context`, `with_relations`, `with_temporal`, `with_limit`, `with_offset`, `with_cursor_after`, `with_order`, `with_hybrid_weights`, and `with_explanation`.

Non-semantic ordering uses `QueryOrder`: `CreatedAtAsc` (the default), `CreatedAtDesc`, `UpdatedAtAsc`, `UpdatedAtDesc`, `ExpressionAsc`, or `ExpressionDesc`. UUID breaks ties deterministically. A query cannot combine cursor and offset pagination.

`QueryResult` contains `entry`, optional `similarity_score`, optional normalized `lexical_score`, optional `combined_score`, and optional human-readable `explanation` plus a compatibility copy of `QueryPlan` when explanation is enabled. `execute` returns `QueryExecution { results, plan }` even when no rows match. Its typed steps report the strategy and measured before/after count for SQL/JSON predicates, FTS5, Rust regex scans, graph traversal, linear vector scoring, top-k, deterministic sorting, and pagination. Hybrid weights are valid only for a query combining meaning with `FullText`; weights must be finite, non-negative, and have a positive sum.

## `ContextDB`

```rust
ContextDB::in_memory() -> StorageResult<ContextDB>
ContextDB::new(path) -> StorageResult<ContextDB>
ContextDB::with_backend(backend) -> ContextDB

db.insert(&entry)
db.insert_batch(&entries)
db.get(id)
db.query(&query)
db.execute(&query)
db.update(&entry)
db.update_batch(&entries)
db.delete(id)
db.delete_batch(&ids)
db.count()

db.integrity_check()
db.backup_to(path)
ContextDB::restore(backup, destination)
db.embedding_profile()
db.set_embedding_profile(&profile)
db.adopt_legacy_embedding_profile(&profile)
db.migrate_embeddings(&profile, &replacements)
db.revisions(id)
db.create_context_index("/project/id")
db.backend_name()
```

Batch mutations are atomic. Updates and deletes return `StorageError::NotFound` for missing UUIDs. Relations must target existing entries, may not point to the entry itself, and are stored as directed outgoing edges.

`EmbeddingProfile { model, version, dimensions }` records database-wide embedding identity. `set_embedding_profile` configures an empty database and refuses to retroactively label populated unidentified data. `adopt_legacy_embedding_profile` is the explicit attestation path for known legacy vectors. `migrate_embeddings` requires one validated replacement vector for every current entry and changes vectors, timestamps, revision snapshots, dimensions, and profile metadata atomically.

`integrity_check` returns an `IntegrityReport` covering SQLite, foreign-key, entry decoding, vector/dimension metadata, revision, and full-text-index problems. `revisions` returns immutable `EntryRevision` snapshots for insert, update, delete, and legacy migration snapshots.

---

| Prev | Next |
| --- | --- |
| [Architecture](architecture.md) | [Performance](performance.md) |
