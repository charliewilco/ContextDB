# Data Schema

ContextDB's current SQLite schema version is 2, stored in `PRAGMA user_version`. Opening a legacy database validates entries and relations, rebuilds relation constraints, records initial revision snapshots, and migrates transactionally. A database from a newer schema version is rejected.

## Tables

`entries` stores `id`, JSON-encoded `meaning` bytes in a BLOB, `expression`, JSON-text `context`, `created_at`, and `updated_at`. Timestamp columns contain RFC3339 strings.

`relations(from_id, to_id)` stores directed outgoing edges. Its composite primary key prevents duplicates, a check rejects self-relations, and foreign keys reference `entries` with `ON DELETE CASCADE`.

`contextdb_metadata(key, value)` stores `vector_dimension`, `embedding_model`, and optional `embedding_model_version`. A dimension without a model represents legacy-unidentified vectors; assigning model identity then requires explicit adoption or complete re-embedding through the public API.

`entry_revisions` stores `revision_id`, `entry_id`, `operation`, the complete entry JSON `snapshot`, and `recorded_at`. Delete revisions intentionally remain after the entry is removed.

`entries_fts` is an FTS5 virtual table containing entry IDs and expressions. Insert/update/delete triggers keep it synchronized.

## Indexes

Built-in indexes cover entry creation/update/expression fields, both relation endpoints, and revision history. `create_context_index("/project/id")` creates a deterministic SQLite expression index on the corresponding `json_extract(context, ...)` path.

## Entry JSON

```json
{
  "id": "f4fdc8c4-5a4e-4d92-9b9b-9a2a0cc8b3c3",
  "meaning": [0.1, 0.2, 0.3],
  "expression": "User prefers cold brew coffee",
  "context": {"category": "dietary", "confidence": 0.9},
  "created_at": "2026-01-15T10:30:00Z",
  "updated_at": "2026-01-15T10:30:00Z",
  "relations": []
}
```

Direct SQL changes bypass API validation and revision recording. Treat the schema as an implementation detail unless performing a controlled recovery or migration.

---

| Prev | Next |
| --- | --- |
| [Data Portability](data-portability.md) | [Security & Privacy](security.md) |
