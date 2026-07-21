# Changelog

All notable changes to ContextDB will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-07-21

### Fixed
- Preserve invalid-argument, not-found, database, and panic error categories through the C and Swift APIs

## [0.1.0] - 2026-07-21

### Added
- Atomic insert, update, and delete batch APIs
- Schema version 2 with validated transactional migration
- FTS5 full-text retrieval, normalized BM25 scores, and weighted hybrid ranking
- Deterministic query ordering, offset pagination, and cursor continuation
- Database-wide embedding profiles and vector-dimension enforcement
- Explicit legacy-profile adoption and atomic complete re-embedding migration
- Context JSON expression indexes
- Entry revision history
- Integrity checking and SQLite online backup/restore
- CLI `add`, integrity, backup/restore, embedding-profile, and revision commands plus atomic JSON import
- C ABI version 1 JSON operations for insert, get, update, delete, and query
- Typed Swift package with a checksum-addressed XCFramework for iOS and macOS
- Typed query execution plans with measured stages, ranking, ordering, and pagination
- Expanded documentation and examples

### Fixed
- Regex filters now evaluate actual patterns without a literal SQL prefilter
- Non-finite, empty, and mixed-dimension vectors are rejected
- Missing updates and deletes report `NotFound`
- Mutations are transactional and relation foreign keys are enforced
- Relation semantics are consistently directed
- CLI offset pagination is applied and Unicode output truncation is safe
- FFI operations contain Rust panics and return structured status codes
- Documentation now distinguishes implemented behavior from future work

## Initial development

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
- Update and delete operations
- Comprehensive test suite
- Demo example showcasing all features
- Documentation (README, CONCEPTS, CONTRIBUTING)

### Design Decisions
- Embedded library (no server required)
- Co-equal representations of semantic and linguistic data
- Schema-less JSON metadata with typed query builders
- Linear scan for vector search (HNSW planned for future)

### Known Limitations
- No HNSW index (linear scan for vector search)
- Single-threaded query execution
- Not optimized for > 100k entries

[Unreleased]: https://github.com/charliewilco/contextdb/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/charliewilco/contextdb/releases/tag/v0.1.1
[0.1.0]: https://github.com/charliewilco/contextdb/releases/tag/v0.1.0
