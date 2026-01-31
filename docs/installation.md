# Installation

This guide covers the three main ways to install ContextDB: Homebrew (macOS), the install script, or Cargo.

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

## Notes

- The `contextdb` CLI is gated behind the `cli` feature when building from source or installing via Cargo.
- The library can be used directly from Rust without the CLI:

```rust
use contextdb::ContextDB;

let mut db = ContextDB::in_memory()?;
```
