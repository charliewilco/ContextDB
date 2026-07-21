# Using ContextDB on iOS (C FFI)

ContextDB can build as a static library with the `ffi` feature. The repository's canonical ABI declarations are in `include/contextdb.h`; use that header rather than copying declarations into an app.

## Build

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

The repository includes a typed Swift package over the C ABI. Build its local binary
target, then use SwiftPM normally:

```bash
./scripts/build_xcframework.sh
swift build
swift test
```

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
This checkout uses a local `dist/ContextDB.xcframework` binary target; a clean clone
must build that artifact before `swift build`. A remotely downloadable,
checksum-addressed Swift package still requires publishing a release artifact.

## ABI v1

Check `contextdb_abi_version()` before using an unfamiliar binary. ABI v1 includes open/close, count, legacy insert/search helpers, and JSON operations for insert, get, update, delete, and the complete serialized Rust `Query`/`QueryResult` surface:

- `contextdb_insert_json`
- `contextdb_get_json`
- `contextdb_update_json`
- `contextdb_delete_id`
- `contextdb_query_json`

JSON calls return `CONTEXTDB_STATUS_OK`, `INVALID_ARGUMENT`, `NOT_FOUND`, `DATABASE`, or `PANIC`. Read `contextdb_last_error_code()` and copy/free `contextdb_last_error_message()` as needed. All returned C strings must be released with `contextdb_string_free`; legacy result arrays must be released with `contextdb_query_results_free` using the exact returned length.

Database operations contain Rust panics and convert them to the panic status or the documented fallback. Deallocation functions still require exactly the pointer and length returned by ContextDB; invalid foreign pointers are undefined behavior and cannot be repaired by panic containment. Pointer lifetime and thread coordination remain the caller's responsibility. A handle must not be used after close, and mutable operations must not race on the same handle.

The C ABI and synchronous Swift wrapper are implemented. ContextDB has not yet
published a remote Swift binary release, and the wrapper does not currently expose
an async or actor-based concurrency layer.

---

| Prev | Next |
| --- | --- |
| [Installation](installation.md) | [Quickstart](quickstart.md) |
