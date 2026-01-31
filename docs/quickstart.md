# Quickstart

A friendly walk-through to get ContextDB running quickly. This guide shows both the CLI and Rust library paths.

## When to use this guide

- You want a working database in minutes.
- You prefer copy-paste steps over deep explanations.
- You are okay using small example vectors instead of real embeddings.

## CLI quickstart (5 minutes)

Embeddings are user-supplied; this example uses a tiny 3-D vector for clarity.

```bash
# Install the CLI
cargo install contextdb --features cli

# Create a database
contextdb init mydata.db

# Insert one entry (via import)
python - <<'PY'
import json, uuid, datetime
now = datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")
entry = {
    "id": str(uuid.uuid4()),
    "meaning": [0.1, 0.2, 0.3],
    "expression": "User doesn't like red onions",
    "context": None,
    "created_at": now,
    "updated_at": now,
    "relations": [],
}
with open("entry.json", "w") as f:
    json.dump([entry], f)
PY
contextdb import mydata.db entry.json

# Query by text
contextdb search mydata.db "onion"
```

### CLI commands you will use most

```bash
# Create a new database
contextdb init mydata.db

# Check database stats
contextdb stats mydata.db

# Search for entries
contextdb search mydata.db "search term"

# List all entries
contextdb list mydata.db

# Show entry details
contextdb show mydata.db <entry-id>

# Interactive mode
contextdb repl mydata.db
```

### Sample dataset

```bash
# Import sample data
contextdb import sample.db examples/sample-data.json

# Explore entries
contextdb list sample.db
contextdb search sample.db "proposal"
contextdb recent sample.db 3
```

### Import/export format

`contextdb export` writes a JSON array of `Entry` objects. `contextdb import` expects the same format.

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

## Rust quick start

```rust
use contextdb::{ContextDB, Entry, Query, ExpressionFilter};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory database (no persistence)
    let mut db = ContextDB::in_memory()?;

    // Or create file-backed database
    // let mut db = ContextDB::new("memories.db")?;

    // Insert entry with semantic + linguistic representation
    let entry = Entry::new(
        vec![0.8, 0.1, 0.3],  // Embedding from your model
        "User doesn't like red onions".to_string(),
    ).with_context(json!({
        "category": "dietary",
        "specificity": "item-level"
    }));

    db.insert(&entry)?;

    // Query by semantic similarity (LLM use case)
    let semantic_results = db.query(
        &Query::new()
            .with_meaning(vec![0.75, 0.15, 0.25], Some(0.7))
            .with_limit(5)
    )?;

    for result in semantic_results {
        println!("Found: {} (similarity: {:.1}%)",
            result.entry.expression,
            result.similarity_score.unwrap() * 100.0
        );
    }

    // Query by text (human inspection)
    let text_results = db.query(
        &Query::new()
            .with_expression(ExpressionFilter::Contains("onion".to_string()))
    )?;

    for result in text_results {
        println!("Found: {}", result.entry.expression);
    }

    Ok(())
}
```

## Common pitfalls

- Embedding vector lengths must match for meaningful similarity results.
- The CLI import format expects a JSON array, not a single object.
- If you are using the CLI from source, you must enable the `cli` feature.

## Next steps

- `embeddings.md` for generating real embeddings
- `query-language.md` for richer filters
- `usage.md` for realistic workflows
