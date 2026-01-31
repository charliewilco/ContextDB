# Why ContextDB?

## TL;DR

ContextDB is an embedded database for semantic data that keeps human-readable text and vector meaning as equals. You can inspect what the system knows, not just what it scores.

## When this is a good fit

- You need a local, embeddable store (no server).
- You want explainable results and human inspection.
- You are prototyping LLM memory or retrieval workflows.

## When it is not a good fit (yet)

- You need large-scale vector search or multi-tenant isolation.
- You require distributed indexing or high write throughput.

## The Problem

Building LLM applications requires storing and retrieving context/memory. Existing solutions fall short:

| Solution | Problem |
|----------|---------|
| **Vector DBs** (Pinecone, ChromaDB) | Can't easily inspect what's stored. Debugging = "why did it retrieve *that*?" |
| **Traditional DBs** (Postgres+pgvector) | Vector search bolted on, no unified query model |
| **Dual Storage** | Sync headaches, consistency issues, 2x complexity |

## The ContextDB Solution

**One database, two equal interfaces:**

```
┌────────────────────────────────────┐
│        ContextDB Entry             │
│                                    │
│  Meaning (vector)  ←→  Expression  │
│  For LLMs              For Humans  │
│                                    │
│  Semantic search   ←→  Text query  │
│  Graph relations   ←→  Metadata    │
│  Similarity rank   ←→  Inspection  │
└────────────────────────────────────┘
```

**Key Benefits:**

- ✅ **Embedded** - No server, no service, just `use contextdb`
- ✅ **Transparent** - Humans can inspect what AI "knows"
- ✅ **Explainable** - Built-in query explanations
- ✅ **Flexible** - Schema-less JSON metadata
- ✅ **Unified** - One query API for semantic + structured + text + temporal
- ✅ **Type-safe** - Compile-time query validation
