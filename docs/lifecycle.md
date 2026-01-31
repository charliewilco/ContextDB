# Entry Lifecycle

Create, update, and delete entries directly through the API:

```rust
use contextdb::{ContextDB, Entry};
use serde_json::json;

let mut db = ContextDB::new("memories.db")?;

let entry = Entry::new(vec![0.2, 0.3, 0.4], "Initial note".to_string())
    .with_context(json!({"category": "note"}));
db.insert(&entry)?;

// Update the entry in place
let mut updated = db.get(entry.id)?;
updated.expression = "Revised note".to_string();
updated.context = json!({"category": "note", "status": "edited"});
db.update(&updated)?;

// Delete when no longer needed
db.delete(entry.id)?;
```
