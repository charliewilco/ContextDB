# Architecture

ContextDB is a synchronous embedded Rust library. `ContextDB` delegates to the `StorageBackend` trait; SQLite is the only implemented backend.

## SQLite storage

- `entries` stores JSON-encoded vector bytes in a BLOB, expression text, JSON context, and timestamps.
- `relations` stores directed edges with foreign keys, cascade deletion, and a self-edge check.
- `entries_fts` is an FTS5 index maintained by triggers.
- `contextdb_metadata` stores the vector dimension and optional embedding model identity.
- `entry_revisions` stores immutable JSON snapshots at mutation boundaries.

File-backed databases use WAL journaling, `synchronous=NORMAL`, foreign-key enforcement, and a 5-second busy timeout. Schema version 2 is recorded with `PRAGMA user_version`; legacy databases are validated and migrated transactionally. Databases created by a newer unsupported schema version are rejected.

## Mutations

Single and batch inserts, updates, and deletes use SQLite transactions. Validation happens before commit: vectors must be finite, non-empty, and dimensionally consistent; relation targets must exist and cannot be self-relations. Updates and deletes require the entry to exist. Each successful mutation records a revision snapshot.

## Query execution

FTS5 supplies BM25 lexical candidates and scores for `FullText`. Other expression, context, temporal, and relation filters narrow the candidate set. Semantic retrieval computes cosine similarity in process with a linear scan. Results are sorted by semantic, lexical, hybrid, or explicit deterministic field ordering before cursor/offset and limit are applied.

`execute` records typed execution steps as they happen, including the actual strategy and candidate counts before and after each stage. It returns a plan even for zero-result queries. `query` remains the compatibility API and copies the plan onto each result only when explanation is requested.

Context JSON paths use JSON Pointer in the Rust API. `create_context_index` converts a pointer into a SQLite `json_extract` expression index for selective application-defined paths.

## Operational APIs

SQLite's online backup API creates consistent snapshots and restores only to a destination that does not already exist. Integrity checks run SQLite quick and foreign-key checks, the native FTS5 integrity command, canonical entry/index comparisons, and validation of stored vectors, embedding metadata, and revisions. These facilities improve embedded durability, but they do not provide replication, a network service, multi-process coordination, or an ANN vector index.
