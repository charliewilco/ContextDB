# Glossary

## Overview
Short definitions of core ContextDB terms.

## When to use
- You want quick clarification of terms.
- You need consistent language across docs or code.

## Examples

**Entry**
- The basic record stored in ContextDB.
- It includes `meaning`, `expression`, `context`, timestamps, and `relations`.

**Meaning**
- The vector embedding for an entry.
- Used for semantic similarity queries.

**Expression**
- The human-readable text for an entry.
- Used for text search and inspection.

**Context**
- A JSON value attached to an entry.
- Used for metadata filtering and debugging.

**Relations**
- Links between entries (by ID).
- Used for graph-style traversal and grouping.

**Filters**
- Constraints applied during queries.
- Includes meaning filters, expression filters, context filters, relation filters, and temporal filters.

## Pitfalls
- Mixing meanings of terms can lead to confusing schemas and queries.
- Treating `expression` as optional makes debugging harder.

## Next steps
- See `concepts.md` for deeper context.
- See `query-language.md` for filter usage.
---

| Prev | Next |
| --- | --- |
| [Query Language](query-language.md) | [Entry Lifecycle](lifecycle.md) |
