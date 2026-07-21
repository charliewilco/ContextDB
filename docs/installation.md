# Installation

## Overview
ContextDB has no published crate, tag, release binary, or Homebrew tap yet. Install the pre-alpha CLI from the current Git main branch or build a checkout.

## When to use
- You are setting up a new environment.
- You want a reproducible install method.

## Examples

## When to use which option

- Local Homebrew formula: convenient for testing a checkout on macOS.
- Install script: a wrapper around Cargo's Git installation; Rust is required.
- Cargo: install directly from the current Git main branch.
- From source: best for contributing or hacking on the codebase.

## Option A: local Homebrew formula (macOS)

```sh
git clone https://github.com/charliewilco/contextdb.git
cd contextdb
brew install --HEAD ./Formula/contextdb.rb
```

## Option B: install script (macOS/Linux with Rust)

```sh
curl -fsSL https://raw.githubusercontent.com/charliewilco/contextdb/main/scripts/install.sh | bash
```

## Option C: Cargo from Git (all platforms with Rust)

```sh
cargo install --git https://github.com/charliewilco/contextdb \
	--branch main --locked --features cli --bin contextdb
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

## Pitfalls
- The CLI requires the `cli` feature when building from source or Cargo.
- These commands follow a moving pre-alpha branch. Pin a commit in production-like evaluation environments.

## Next steps
- Run `contextdb --help` to verify.
- Continue with `quickstart.md`.
---

| Prev | Next |
| --- | --- |
| [Why ContextDB?](why.md) | [iOS](ios.md) |
