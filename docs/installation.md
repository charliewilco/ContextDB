# Installation

## Overview
Install ContextDB via Homebrew, script, Cargo, or source.

## When to use
- You are setting up a new environment.
- You want a reproducible install method.

## Examples

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

## Pitfalls
- The CLI requires the `cli` feature when building from source or Cargo.

## Next steps
- Run `contextdb --help` to verify.
- Continue with `quickstart.md`.
---

| Prev | Next |
| --- | --- |
| [Why ContextDB?](why.md) | [iOS](ios.md) |
