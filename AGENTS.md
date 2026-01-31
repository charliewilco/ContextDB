# Repository Guidelines

## Project Structure & Module Organization
- `src/` holds the Rust library and CLI entry points (`lib.rs`, `types.rs`, `query.rs`, `storage/`, `bin/main.rs`).
- `examples/` contains runnable demos (e.g., `demo.rs`, `backends.rs`).
- `scripts/` includes helper scripts like `install.sh`.
- `Formula/` contains the Homebrew formula (`contextdb.rb`).
- Docs live at the root (`README.md`, `docs/architecture.md`, `docs/concepts.md`, `docs/embeddings.md`, `CHANGELOG.md`).

## Build, Test, and Development Commands
- `cargo build` builds the library and CLI locally.
- `cargo test` runs the unit and integration tests.
- `cargo run --example demo` runs the demo workflow from `examples/`.
- `cargo fmt --check` verifies formatting; use `cargo fmt` to apply.
- `cargo clippy -- -D warnings` runs lints and treats warnings as errors.

## Coding Style & Naming Conventions
- Use standard Rust style (`rustfmt`) with 4-space indentation and trailing commas.
- Prefer idiomatic naming: `snake_case` for functions/variables, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Keep public APIs documented with Rust doc comments (`///`) and minimal examples when helpful.

## Testing Guidelines
- Tests live alongside code in `src/` modules with `#[cfg(test)]`, plus any future `tests/` integration suites.
- Name tests descriptively (e.g., `test_query_with_expression_filter`).
- Run `cargo test` before opening a PR; use `cargo test -- --nocapture` when debugging output.

## Commit & Pull Request Guidelines
- Use Conventional Commits (e.g., `feat: add hnsw index`, `fix: handle empty query`).
- PRs should include: a clear description, motivation, a concise change list, and how you tested.
- Ensure formatting and linting are clean (`cargo fmt`, `cargo clippy`) and update docs if behavior changes.

## Architecture References
- For system design details, see `docs/architecture.md` and `docs/concepts.md`.
- The storage backend is currently SQLite (`src/storage/sqlite.rs`) and vector similarity is linear scan.
