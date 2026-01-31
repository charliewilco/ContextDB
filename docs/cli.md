# CLI Guide

## Overview
Complete reference for the `contextdb` CLI.

## When to use
- You want command/flag details.
- You are scripting or automating tasks.

## Examples

## Install the CLI

The CLI is behind the `cli` feature flag:

```sh
cargo install contextdb --features cli
```

## Global help

```sh
contextdb --help
contextdb --version
```

## Command reference

### `init` - Create a new database

```sh
contextdb init [path]
```

- `path` (optional): database file path. Defaults to `contextdb.db`.

Example:

```sh
contextdb init mydata.db
```

### `stats` - Show database statistics

```sh
contextdb stats <path>
```

Example:

```sh
contextdb stats mydata.db
```

### `search` - Search entries by text

```sh
contextdb search <path> <query> [--limit <n>] [--format <table|json|plain>]
```

Flags:
- `-l, --limit`: max results (default `10`)
- `-f, --format`: output format (default `table`)

Examples:

```sh
contextdb search mydata.db "onion"
contextdb search mydata.db "coffee" --limit 5 --format json
```

### `list` - List entries

```sh
contextdb list <path> [--limit <n>] [--offset <n>] [--format <table|json|plain>]
```

Flags:
- `-l, --limit`: max entries (default `20`)
- `-o, --offset`: offset for pagination (default `0`)
- `-f, --format`: output format (default `table`)

Examples:

```sh
contextdb list mydata.db
contextdb list mydata.db --limit 50 --format plain
```

Note: the current implementation ignores `--offset` and always starts from the beginning.

### `show` - Show a specific entry

```sh
contextdb show <path> <id>
```

`id` can be a full UUID or a unique prefix.

Example:

```sh
contextdb show mydata.db 4e2a1c8b
```

### `export` - Export database to JSON

```sh
contextdb export <path> [--output <file>]
```

Flags:
- `-o, --output`: write to file (stdout if omitted)

Example:

```sh
contextdb export mydata.db --output backup.json
```

### `import` - Import entries from JSON

```sh
contextdb import <path> <input>
```

Example:

```sh
contextdb import mydata.db entries.json
```

### `delete` - Delete an entry

```sh
contextdb delete <path> <id> [--force]
```

Flags:
- `-f, --force`: skip confirmation prompt

Example:

```sh
contextdb delete mydata.db 4e2a1c8b
contextdb delete mydata.db 4e2a1c8b --force
```

### `recent` - Show recent entries

```sh
contextdb recent <path> [--count <n>]
```

Flags:
- `-c, --count`: number of entries (default `10`)

Example:

```sh
contextdb recent mydata.db --count 5
```

### `repl` - Interactive REPL

```sh
contextdb repl <path>
```

The REPL lets you inspect a database without repeated CLI invocations.

Commands:

- `help`, `h`, `?` - Show help
- `search <query>` - Search by text
- `list [n]` - List entries (default 10)
- `show <id>` - Show entry details
- `stats` - Show count
- `recent [n]` - Show most recent
- `quit`, `exit`, `q` - Exit

Examples:

```
$ contextdb repl mydata.db
ContextDB REPL
Database: mydata.db (42 entries)
Type 'help' for commands, 'quit' to exit

contextdb> search coffee
...
```

## Import/export format

`contextdb export` writes a JSON array of `Entry` objects. `contextdb import` expects the same format.

## Pitfalls
- `--offset` is accepted but not yet applied.
- Use a unique ID prefix for `show` and `delete`.

## Next steps
- See `data-portability.md` for backup flows.
- See `quickstart.md` for a short walkthrough.
