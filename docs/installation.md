# Installation

This guide covers the main ways to install ContextDB. Pick the option that matches your environment and preferences.

## When to use which option

- Homebrew: best for macOS (and Linuxbrew) users who want upgrades managed by brew.
- Install script: quick install for macOS or Linux without Rust tooling.
- Cargo: best if you already use Rust and want reproducible builds.
- From source: best for contributing or hacking on the codebase.

## Option A: Homebrew (macOS)

```sh
brew tap charliewilco/contextdb
brew install contextdb
```

## Option B: Install script (macOS/Linux)

```sh
curl -fsSL https://raw.githubusercontent.com/charliewilco/contextdb/main/scripts/install.sh | bash
```

## Option C: Cargo (all platforms with Rust)

```sh
cargo install contextdb --features cli
```

## Build from source

```sh
git clone https://github.com/charliewilco/contextdb.git
cd contextdb
cargo run --features cli --bin contextdb -- --help
```

## Verify install

```sh
contextdb --version
contextdb --help
```

## Notes and tips

- The `contextdb` CLI is gated behind the `cli` feature when building from source or installing via Cargo.
- The Rust library can be used directly without the CLI.

```rust
use contextdb::ContextDB;

let mut db = ContextDB::in_memory()?;
```
