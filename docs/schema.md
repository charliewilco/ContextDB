# Data Schema

## Overview
This guide explains the SQLite schema and JSON shapes that ContextDB uses.

## When to use
- You are inspecting the database directly in SQL tools.
- You need to export, validate, or transform data.
- You are building integrations around the raw `.db` file.

## Examples

## Tables

ContextDB stores data in two SQLite tables: `entries` and `relations`.

### `entries`

```sql
CREATE TABLE IF NOT EXISTS entries (
    id TEXT PRIMARY KEY,
    meaning BLOB NOT NULL,
    expression TEXT NOT NULL,
    context TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Column details:
- `id`: UUID string.
- `meaning`: BLOB containing a bincode-serialized `Vec<f32>`.
- `expression`: human-readable text.
- `context`: JSON as text.
- `created_at`: RFC3339 timestamp string.
- `updated_at`: RFC3339 timestamp string.

### `relations`

```sql
CREATE TABLE IF NOT EXISTS relations (
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id),
    FOREIGN KEY (from_id) REFERENCES entries(id),
    FOREIGN KEY (to_id) REFERENCES entries(id)
);
```

### Indexes

```sql
CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at);
CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(updated_at);
CREATE INDEX IF NOT EXISTS idx_entries_expression ON entries(expression);
CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(from_id);
CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(to_id);
```

## JSON export/import shape

`contextdb export` writes a JSON array of full `Entry` objects. `contextdb import` expects the same format.

```json
[
  {
    "id": "f4fdc8c4-5a4e-4d92-9b9b-9a2a0cc8b3c3",
    "meaning": [0.1, 0.2, 0.3],
    "expression": "User prefers cold brew coffee",
    "context": {"category": "dietary", "confidence": 0.9},
    "created_at": "2026-01-15T10:30:00Z",
    "updated_at": "2026-01-15T10:30:00Z",
    "relations": []
  }
]
```

## Pitfalls
- `meaning` is a binary blob in SQLite; do not treat it as JSON or text.
- `context` must be valid JSON if you edit it directly.
- Timestamps should be RFC3339 strings for compatibility.

## Next steps
- See `base.md` for GUI inspection.
- See `data-portability.md` for export/import workflows.
