# Contributing to ContextDB

Thanks for your interest in contributing! This is an exploratory project, and we welcome contributions of all kinds.

## Quick Start

```bash
# Fork and clone the repo
git clone https://github.com/charliewilco/contextdb.git
cd contextdb

# Run tests
cargo test

# Run the demo
cargo run --example demo

# Check your changes
cargo fmt --check
cargo clippy
```

## Areas for Contribution

### 🚀 Performance
- HNSW index integration
- Query optimization
- Parallel vector operations
- Benchmarking suite

### ✨ Features
- GraphQL API for human inspection
- Query language parser (text-based queries)
- Batch operations
- Query builder conveniences (relations/top_k)
- HTTP server mode

### 📚 Documentation
- More examples and tutorials
- Integration guides (LangChain, LlamaIndex, etc.)
- Architecture deep-dives
- Video walkthroughs

### 🧪 Testing
- Property-based tests
- Stress tests
- Edge case coverage
- Benchmarks

### 🔌 Integrations
- Python client library
- JavaScript client library
- Swift client library
- LangChain integration
- LlamaIndex integration

## Development Guidelines

### Code Style

We follow standard Rust conventions:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Run tests
cargo test
```

### Commit Messages

Use conventional commits:

```
feat: add HNSW index support
fix: correct similarity calculation edge case
docs: improve quick start guide
test: add benchmarks for vector search
refactor: simplify query execution
```

### Pull Request Process

1. **Fork the repo** and create a feature branch
2. **Make your changes** with clear, focused commits
3. **Add tests** for new functionality
4. **Update docs** if needed (README, doc comments)
5. **Run checks**: `cargo test && cargo fmt && cargo clippy`
6. **Open a PR** with a clear description of the changes

### PR Template

```markdown
## Description
Brief description of the changes

## Motivation
Why is this change needed?

## Changes
- Change 1
- Change 2

## Testing
How was this tested?

## Checklist
- [ ] Tests pass
- [ ] Code formatted (`cargo fmt`)
- [ ] No clippy warnings
- [ ] Documentation updated
```

## Architecture Overview

See [CONCEPTS.md](CONCEPTS.md) for detailed architecture explanation.

Key components:

```
src/
├── lib.rs          # Public API
├── types.rs        # Entry, cosine_similarity
├── query.rs        # Query types and filters
└── storage/        # Backends (sqlite) + StorageBackend trait
```

## Testing Strategy

### Unit Tests

Test individual components:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entry_creation() {
        let entry = Entry::new(vec![0.1, 0.2], "test".to_string());
        assert_eq!(entry.expression, "test");
    }
}
```

### Integration Tests

Test full workflows in `tests/`:

```bash
# Create tests/integration_test.rs
cargo test --test integration_test
```

### Benchmarks

Add benchmarks in `benches/`:

```bash
cargo bench
```

## Documentation

### Doc Comments

Use Rust doc comments:

```rust
/// Create a new entry with the given meaning and expression
///
/// # Arguments
///
/// * `meaning` - The vector embedding
/// * `expression` - Human-readable text
///
/// # Examples
///
/// ```
/// let entry = Entry::new(vec![0.1, 0.2], "text".to_string());
/// ```
pub fn new(meaning: Vec<f32>, expression: String) -> Self {
    // ...
}
```

### Examples

Add examples in `examples/`:

```bash
cargo run --example your_example
```

## Release Process

(For maintainers)

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Tag release: `git tag v0.x.0`
4. Push: `git push origin v0.x.0`
5. Publish: `cargo publish` (when ready)

## Questions?

- Open an [issue](https://github.com/charliewilco/contextdb/issues)
- Start a [discussion](https://github.com/charliewilco/contextdb/discussions)

## Code of Conduct

Be respectful, constructive, and collaborative. This is a learning project where ideas are explored.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
