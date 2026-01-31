# Quickstart

```rust
// Create database - no server, no service, just use it
let mut db = ContextDB::in_memory()?;

// Store semantic meaning + human expression together
let entry = Entry::new(
    vec![0.1, 0.2, 0.3],  // your embedding
    "User doesn't like red onions".to_string()
);
db.insert(&entry)?;

// LLMs query by similarity, humans query by text
let results = db.query(&Query::new()
    .with_meaning(query_vector, Some(0.8))
    .with_expression(ExpressionFilter::Contains("onion".to_string()))
)?;
```

Friendly steps to get ContextDB running fast, using either the CLI or Rust.

## CLI quickstart (5 minutes)

Embeddings are user-supplied; this example uses a tiny 3-D vector for illustration.

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

## CLI usage

Quick start:

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

Commands:

| Command | Description |
|---------|-------------|
| `init <path>` | Create a new database |
| `stats <path>` | Show database statistics |
| `search <path> <query>` | Search entries by text |
| `list <path>` | List all entries |
| `show <path> <id>` | Show entry details |
| `recent <path>` | Show recently added entries |
| `export <path>` | Export database to JSON |
| `import <path> <file>` | Import entries from JSON |
| `delete <path> <id>` | Delete an entry |
| `repl <path>` | Interactive REPL mode |

Examples:

```bash
# Search with limit
contextdb search mydata.db "coffee" --limit 5

# Export to file
contextdb export mydata.db --output backup.json

# List in JSON format
contextdb list mydata.db --format json

# Delete with confirmation
contextdb delete mydata.db abc123

# Delete without confirmation
contextdb delete mydata.db abc123 --force
```

Sample dataset:

```bash
# Import sample data
contextdb import sample.db examples/sample-data.json

# Explore entries
contextdb list sample.db
contextdb search sample.db "proposal"
contextdb recent sample.db 3
```

Import/export format:

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

REPL mode:

```
$ contextdb repl mydata.db
ContextDB REPL
Database: mydata.db (42 entries)
Type 'help' for commands, 'quit' to exit

contextdb> search coffee
abc12345 | User prefers cold brew coffee
def67890 | Coffee shop recommendation: Blue Bottle

contextdb> show abc12345
ID: abc12345-...
Expression: User prefers cold brew coffee
Context: {"category": "dietary", "confidence": 0.9}
Created: 2026-01-15 10:30:00

contextdb> recent 5
...

contextdb> quit
Goodbye!
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

## Run the demo

```bash
cargo run --features cli --example demo
```

This shows:
- Dietary preferences with varying granularity
- Semantic search (LLM query pattern)
- Text search (human query pattern)
- Metadata filtering
- Hybrid queries combining multiple filters
- Explainable results

## More examples

```bash
cargo run --features cli --example backends
cargo run --features cli --example relations
cargo run --features cli --example lifecycle
cargo run --features cli --example advanced_queries
```

Examples cover:
- Backend swapping and storage setup
- Relation graphs and traversal queries
- Entry update/delete lifecycle
- Advanced query construction (top_k, temporal, hybrid)
