# Roadmap

The implemented baseline now includes transactional single/batch mutations, vector validation and embedding identity, schema v2 migration, foreign keys, WAL configuration, FTS5/BM25 and hybrid scoring, deterministic pagination, context indexes, revision history, integrity checks, backup/restore, atomic CLI import, and a versioned JSON C ABI.

Remaining work should be driven by measured product needs. The clearest current gaps are:

- an approximate-nearest-neighbor index for vector workloads that outgrow linear scans;
- repeatable benchmark baselines and explicit supported workload envelopes;
- a supported Swift package and ergonomic Swift API over the C ABI;
- explicit connection-pooling or concurrency guidance for higher-write workloads;
- adaptive cost-based planning beyond the current structured execution plan.

PostgreSQL/MySQL backends, a network server, replication, embedding providers, and language clients are possible expansions, not current commitments. Roadmap items are intentions rather than compatibility guarantees.

---

| Prev | Next |
| --- | --- |
| [Performance](performance.md) | [Data Portability](data-portability.md) |
