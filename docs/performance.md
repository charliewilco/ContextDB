# Performance

## Overview
Current performance characteristics and trade-offs.

## When to use
- You are sizing a dataset or planning scale.

## Examples

## Current implementation

- **Storage**: SQLite with bundled library
- **Vector search**: Linear scan with cosine similarity
- **Suitable for**: under ~100,000 entries (rule of thumb)
- **Query latency**:
  - Text/metadata queries: typically < 1ms (indexed)
  - Semantic queries: O(n) - linear in entry count
  - Hybrid queries: sum of component costs

## Practical tips

- Keep embedding dimensions as small as your model allows.
- Prefer metadata or text filters to narrow the candidate set before vector scoring.
- Batch inserts when possible to reduce transaction overhead.

## Optimization roadmap

- [ ] HNSW index for approximate nearest neighbor search
- [ ] Batch insertion API
- [ ] Query result caching
- [ ] Parallel vector comparison
- [ ] Memory-mapped vectors
- [ ] Persistent vector index

## Benchmarks

```bash
cargo bench  # Coming soon
```

## Pitfalls
- Large datasets will slow linear vector scans.

## Next steps
- See `roadmap.md` for planned optimizations.
---

Prev: [API Reference](api-reference.md)
Next: [Roadmap](roadmap.md)
