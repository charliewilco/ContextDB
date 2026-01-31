# Performance

### Current Implementation

- **Storage**: SQLite with bundled library
- **Vector search**: Linear scan with cosine similarity
- **Suitable for**: < 100,000 entries
- **Query latency**: 
  - Text/metadata queries: < 1ms (indexed)
  - Semantic queries: O(n) - linear in entry count
  - Hybrid queries: Sum of components

### Optimization Roadmap

- [ ] HNSW index for approximate nearest neighbor search
- [ ] Batch insertion API
- [ ] Query result caching
- [ ] Parallel vector comparison
- [ ] Memory-mapped vectors
- [ ] Persistent vector index

### Benchmarks

```bash
cargo bench  # (Coming soon)
```
