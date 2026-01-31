# Security and Privacy

## Overview
Guidance on handling sensitive data with ContextDB.

## When to use
- You store personal or confidential data.
- You need a privacy review before shipping.
- You want safer defaults for embeddings and logs.

## Examples

## Data classification

Decide what types of data can enter ContextDB. Mark sensitive fields early and avoid storing secrets unless required.

## Redaction and minimization

- Store only what you need.
- Remove or mask identifiers before embedding.
- Prefer references (IDs) over raw content when possible.

## Encryption at rest

ContextDB uses SQLite files. Consider disk encryption or an encrypted container if you store sensitive content. You can also encrypt the database file at the filesystem layer.

## Access control

Treat the `.db` file as sensitive. Limit who can read or copy it. For shared systems, use OS permissions to restrict access.

## Embeddings and leakage

Embeddings can encode sensitive content. Do not assume they are safe to share. Apply the same privacy rules to embeddings as to raw text.

## Logging and exports

- Avoid logging full entries in production logs.
- Treat JSON exports as sensitive backups.
- Rotate and protect export files.

## Pitfalls
- Storing raw secrets (tokens, passwords) in entries.
- Sharing embeddings without consent or review.
- Leaving database files in world-readable locations.

## Next steps
- See `data-portability.md` for export/import hygiene.
- See `schema.md` to understand what is stored.
---

| Prev | Next |
| --- | --- |
| [Data Schema](schema.md) | [FAQ](faq.md) |
