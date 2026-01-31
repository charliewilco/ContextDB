# Using ContextDB with Base (SQLite)

This guide shows how to inspect and lightly manage a ContextDB SQLite file using the Base app.

## 1) Create or locate a ContextDB database

- CLI example:

```sh
contextdb init mydata.db
```

- Rust example:

```rust
let mut db = ContextDB::new("mydata.db")?;
```

## 2) Open the database in Base

Open your `.db` file in Base. Once opened, tables will appear in the sidebar.

## 3) Browse entries

Select a table in the sidebar, then use the Data tab to view rows. Base supports paging, filtering, and inline editing from the data grid. citeturn2view0

## 4) Understand the ContextDB schema

ContextDB uses two tables:

- `entries`
  - `id` (TEXT, UUID)
  - `meaning` (BLOB, bincode-serialized `Vec<f32>`)
  - `expression` (TEXT)
  - `context` (TEXT, JSON string)
  - `created_at` (TEXT, RFC3339)
  - `updated_at` (TEXT, RFC3339)
- `relations`
  - `from_id` (TEXT, UUID)
  - `to_id` (TEXT, UUID)

In Base, the Schema tab shows the column list and the SQL used to create the table. citeturn2view2

## 5) Run SQL queries

Use the SQL tab to run custom queries; Base executes statements in order and shows the results of the last statement. You can also save snippets and export results from this view. citeturn2view1

Example queries:

```sql
-- Count entries
SELECT COUNT(*) FROM entries;

-- Latest entries
SELECT id, expression, created_at
FROM entries
ORDER BY created_at DESC
LIMIT 20;

-- Relations with expressions
SELECT r.from_id, e1.expression AS from_expr, r.to_id, e2.expression AS to_expr
FROM relations r
JOIN entries e1 ON e1.id = r.from_id
JOIN entries e2 ON e2.id = r.to_id
LIMIT 50;
```

## 6) Import/export data

Base can export to SQL, delimited text, JSON, and Excel. SQL is the only format that can export multiple tables at once; other formats export one table or view at a time. citeturn0search0

Base can import SQL files or delimited text (like CSV). Keep in mind that SQL files must be compatible with SQLite’s dialect. citeturn0search1

Note: Import/export requires a Pro unlock in Base. citeturn0search10

## 7) Attach other databases (optional)

You can attach another SQLite file using the action button in the lower-left and “Attach Database…”. Give the attached database a name, and it will appear in the sidebar with that name. citeturn1view3

Because of macOS sandboxing, Base can only attach databases you have explicitly opened or selected through its UI. `ATTACH DATABASE` in SQL will only work if Base already has access to the file. citeturn1view3

## Notes and gotchas

- The `meaning` column is a bincode-serialized vector; treat it as an opaque BLOB unless you know the encoding.
- The `context` column is JSON stored as text; keep it valid JSON if you edit it.
- If another app modifies the database while Base is open, use Base’s Refresh menu item to reload visible data. citeturn0search10
- Base 3 requires macOS 15 (Sequoia) or newer. citeturn0search10
