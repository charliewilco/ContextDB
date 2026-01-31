# Data Portability

## Overview
Move databases and data between environments.

## When to use
- You need backups or migrations.

## Examples

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

## Pitfalls
- `import` expects a JSON array.

## Next steps
- See `cli.md` for export/import commands.
---

| Prev | Next |
| --- | --- |
| [Roadmap](roadmap.md) | [Data Schema](schema.md) |
