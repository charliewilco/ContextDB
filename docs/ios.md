# Using ContextDB on Apple Platforms

ContextDB v0.1.1 provides a typed Swift package for iOS 15 or later and macOS 12
or later. Add `https://github.com/charliewilco/ContextDB` as a Swift package
dependency and select version `0.1.1`.

```swift
.package(
	url: "https://github.com/charliewilco/ContextDB",
	exact: "0.1.1"
)
```

The package downloads a checksum-verified XCFramework containing the Rust core and
links the public `ContextDB` Swift library over its versioned C ABI.

## Use the Swift API

```swift
import ContextDB

let database = try ContextDatabase(path: databaseURL)
let entry = try database.insert(
	expression: "Swift-native access",
	meaning: [0.2, 0.8],
	context: .object(["platform": .string("iOS")])
)
let matches = try database.query(
	Query(
		expression: .fullText("Swift"),
		context: .pathEquals("/platform", .string("iOS"))
	)
)
```

`ContextDatabase`, `Entry`, `Query`, the filter types, and `JSONValue` are public
Swift types. The wrapper serializes the typed API through the versioned JSON C ABI.

## Build the binary package from source

ContextDB can build as a static library with the `ffi` feature. The repository's
canonical ABI declarations are in `include/contextdb.h`; use that header rather
than copying declarations into an app.

The checked-in builder produces an iOS device slice plus universal Apple Silicon/Intel
simulator and macOS slices with the canonical header and module map:

```sh
./scripts/build_xcframework.sh
```

It writes `dist/ContextDB.xcframework` and refuses to overwrite an existing artifact.
The script pins the package's iOS 15 and macOS 12 deployment targets and supports an
external Cargo build directory through `CARGO_TARGET_DIR`.

Create the release archive and its SwiftPM checksum with:

```sh
./scripts/package_xcframework.sh
```

The packager also refuses to overwrite an existing archive. Publish that exact ZIP,
then use the printed checksum in a remote SwiftPM binary target.

## ABI v1

Check `contextdb_abi_version()` before using an unfamiliar binary. ABI v1 includes open/close, count, legacy insert/search helpers, and JSON operations for insert, get, update, delete, and the complete serialized Rust `Query`/`QueryResult` surface:

- `contextdb_insert_json`
- `contextdb_get_json`
- `contextdb_update_json`
- `contextdb_delete_id`
- `contextdb_query_json`

JSON calls return `CONTEXTDB_STATUS_OK`, `INVALID_ARGUMENT`, `NOT_FOUND`, `DATABASE`, or `PANIC`. Validation failures, including invalid vectors and query parameters, return `INVALID_ARGUMENT`; missing entries or relation targets return `NOT_FOUND`. The Swift wrapper maps these statuses to distinct `ContextDBError` cases. Read `contextdb_last_error_code()` and copy/free `contextdb_last_error_message()` as needed. All returned C strings must be released with `contextdb_string_free`; legacy result arrays must be released with `contextdb_query_results_free` using the exact returned length.

Database operations contain Rust panics and convert them to the panic status or the documented fallback. Deallocation functions still require exactly the pointer and length returned by ContextDB; invalid foreign pointers are undefined behavior and cannot be repaired by panic containment. Pointer lifetime and thread coordination remain the caller's responsibility. A handle must not be used after close, and mutable operations must not race on the same handle.

The C ABI and synchronous Swift wrapper are implemented. The wrapper does not
currently expose an async or actor-based concurrency layer.

---

| Prev | Next |
| --- | --- |
| [Installation](installation.md) | [Quickstart](quickstart.md) |
