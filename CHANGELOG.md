# Changelog

All notable changes to ContextDB will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- HNSW index for approximate nearest neighbor search
- Batch insertion API
- Update and delete operations
- GraphQL API
- HTTP server mode
- Query language parser
- Python client library
- JavaScript client library

## [0.1.0] - 2026-01-20

### Added
- Initial release of ContextDB
- Core `Entry` type with dual representations (meaning + expression)
- Unified `Query` API supporting five modalities:
  - Semantic search (vector similarity)
  - Text search (expression matching)
  - Metadata filtering (JSON context)
  - Graph relationships (relations)
  - Temporal queries (time-based)
- SQLite storage backend
- In-memory and file-backed database options
- Cosine similarity calculation
- Query result explanations
- Comprehensive test suite
- Demo example showcasing all features
- Documentation (README, CONCEPTS, CONTRIBUTING)

### Design Decisions
- Embedded library (no server required)
- Co-equal representations of semantic and linguistic data
- Schema-less JSON metadata with type-safe queries
- Linear scan for vector search (HNSW planned for future)

### Known Limitations
- No HNSW index (linear scan for vector search)
- Insert-only (no updates or deletes)
- Single-threaded query execution
- Not optimized for > 100k entries

[Unreleased]: https://github.com/charliewilco/contextdb/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/charliewilco/contextdb/releases/tag/v0.1.0
