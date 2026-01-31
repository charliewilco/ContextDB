# Data Portability

ContextDB is an embedded SQLite file plus JSON export/import:

- **Database file**: copy the `.db` file anywhere and open it with `ContextDB::new`.
- **JSON export**: `contextdb export my.db --output backup.json`
- **JSON import**: `contextdb import my.db backup.json`

The JSON format is a list of full `Entry` objects, including IDs and timestamps, so round-tripping preserves provenance.
