# Performance

ContextDB uses SQLite for persistence and FTS5/BM25 for full-text retrieval. Semantic search deserializes candidate vectors and computes cosine similarity in process, so its cost remains linear in the candidate count and vector dimension.

No dataset-size or latency guarantee is currently published. Measure with your vectors, filters, filesystem, and hardware before choosing a production workload.

Practical guidance:

- Use `FullText` or selective filters to narrow work where the query permits it.
- Create JSON expression indexes for frequently queried context paths with `create_context_index`.
- Use atomic batch mutation APIs to amortize transaction overhead.
- Keep embedding dimensions no larger than the selected model requires.
- Use cursor pagination to continue after the final entry in a deterministically ordered page. The cursor entry must still match the query.

Run the included Criterion benchmarks with:

```sh
cargo bench
```

An indicative local run on July 10, 2026 used 5,000 entries with 128-dimensional vectors, 10 samples, one-second warmup, and one-second measurement windows:

| Operation | Observed interval |
| --- | ---: |
| Atomic batch insert, 1,000 entries | 32.9–33.9 ms |
| Semantic query, 5,000 candidates | 49.5–51.1 ms |
| Substring query, 5,000 entries | 7.4–8.7 ms |
| FTS5 query, 5,000 entries | 6.6–6.8 ms |
| Hybrid FTS5/vector query | 8.1–8.4 ms |
| Indexed context query returning roughly half the dataset | 30.0–31.0 ms |

These numbers are development evidence, not a performance contract. The context result remains comparatively expensive because matching entries and their vectors must still be decoded.

The major remaining scaling limitation is the lack of an approximate-nearest-neighbor index. Caching, parallel scoring, and memory-mapped vector storage are not implemented.

---

| Prev | Next |
| --- | --- |
| [API Reference](api-reference.md) | [Roadmap](roadmap.md) |
