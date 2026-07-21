# Data Portability

ContextDB supports two distinct portability paths.

## SQLite backup and restore

`db.backup_to(path)` uses SQLite's online backup API to create a consistent database snapshot. `ContextDB::restore(backup, destination)` restores the snapshot and refuses to overwrite an existing destination. This preserves the complete database, including metadata, FTS state, revisions, and schema version.

```rust
db.backup_to("contextdb.backup")?;
ContextDB::restore("contextdb.backup", "restored.db")?;
```

Prefer this path for operational backup and disaster recovery. Copying only the main `.db` file while a WAL-backed database is open can miss committed WAL contents.

## JSON export and import

```sh
contextdb export my.db --output entries.json
contextdb import restored.db entries.json
```

JSON is a portable array of complete `Entry` objects and preserves entry UUIDs, timestamps, context, vectors, and relations. Import is atomic. It does not carry database-wide embedding-profile metadata or pre-existing revision history; imported entries begin new insert revisions.

Run `integrity_check` after moving or restoring databases when the application needs explicit verification.

---

| Prev | Next |
| --- | --- |
| [Roadmap](roadmap.md) | [Data Schema](schema.md) |
