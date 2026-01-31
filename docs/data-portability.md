# Data Portability

ContextDB is an embedded SQLite file plus JSON export/import. This means you can move data around with plain files.

## When to use this guide

- You need backups or migrations between environments.
- You want to inspect or version data outside of ContextDB.
- You are troubleshooting import/export issues.

## What is portable

- **Database file**: copy the `.db` file anywhere and open it with `ContextDB::new`.
- **JSON export**: `contextdb export my.db --output backup.json`
- **JSON import**: `contextdb import my.db backup.json`

The JSON format is a list of full `Entry` objects, including IDs and timestamps, so round-tripping preserves provenance.

## Example workflow

```bash
# Export to JSON
contextdb export my.db --output backup.json

# Restore into a new database
contextdb init restored.db
contextdb import restored.db backup.json
```

## Common pitfalls

- `contextdb import` expects a JSON array, not a single object.
- If you edit JSON by hand, keep timestamps in RFC3339 format.
