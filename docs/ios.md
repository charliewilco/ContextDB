# Using ContextDB on iOS (FFI)

## Overview
Use ContextDB from iOS via the C FFI layer.

## When to use
- You are embedding ContextDB in an iOS app.

## Examples

## 1) Build the static libraries

Install Rust targets for iOS device and simulator:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

Build the static libraries with the `ffi` feature:

```sh
cargo build --release --features ffi --target aarch64-apple-ios
cargo build --release --features ffi --target aarch64-apple-ios-sim
cargo build --release --features ffi --target x86_64-apple-ios
```

## 2) Create an XCFramework

```sh
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libcontextdb.a \
  -library target/aarch64-apple-ios-sim/release/libcontextdb.a \
  -library target/x86_64-apple-ios/release/libcontextdb.a \
  -output ContextDB.xcframework
```

If you only target Apple‑silicon simulators, you can omit the `x86_64-apple-ios` slice.

## 3) Add a C header

Create a header file in your app target (e.g. `ContextDBFFI.h`) and add the FFI declarations:

```c
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct ContextDBHandle ContextDBHandle;

typedef struct {
    uint8_t id[16];
    float score;
    char *expression;
} ContextDBQueryResult;

char *contextdb_last_error_message(void);
void contextdb_string_free(char *ptr);

ContextDBHandle *contextdb_open(const char *path);
void contextdb_close(ContextDBHandle *handle);

bool contextdb_insert(ContextDBHandle *handle,
                      const char *expression,
                      const float *meaning_ptr,
                      size_t meaning_len);

bool contextdb_count(const ContextDBHandle *handle, size_t *out_count);

ContextDBQueryResult *contextdb_query_meaning(const ContextDBHandle *handle,
                                              const float *meaning_ptr,
                                              size_t meaning_len,
                                              float threshold,
                                              size_t limit,
                                              size_t *out_len);

ContextDBQueryResult *contextdb_query_expression_contains(const ContextDBHandle *handle,
                                                          const char *expression,
                                                          size_t limit,
                                                          size_t *out_len);

void contextdb_query_results_free(ContextDBQueryResult *results, size_t len);
```

Expose this header to Swift via a bridging header or module map.

## 4) Add the XCFramework to Xcode

Drag `ContextDB.xcframework` into your project and ensure it’s linked in the target’s “Frameworks, Libraries, and Embedded Content”.

## 5) Use from Swift (minimal example)

```swift
import Foundation

func openDatabasePath() -> String {
    let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("contextdb.db")
    return url.path
}

let handle = openDatabasePath().withCString { path in
    contextdb_open(path)
}

guard let db = handle else {
    if let err = contextdb_last_error_message() {
        let message = String(cString: err)
        contextdb_string_free(err)
        print("ContextDB error:", message)
    }
    fatalError("Failed to open DB")
}

let vector: [Float] = [0.1, 0.2, 0.3]
let expression = "hello iOS"
expression.withCString { expr in
    vector.withUnsafeBufferPointer { buf in
        _ = contextdb_insert(db, expr, buf.baseAddress, buf.count)
    }
}

var outLen: Int = 0
let results = vector.withUnsafeBufferPointer { buf in
    contextdb_query_meaning(db, buf.baseAddress, buf.count, 0.0, 10, &outLen)
}

if let results {
    for i in 0..<outLen {
        let item = results.advanced(by: i).pointee
        if let expr = item.expression {
            print(String(cString: expr), item.score)
        }
    }
    contextdb_query_results_free(results, outLen)
}

contextdb_close(db)
```

## Pitfalls
- Always free FFI strings and result buffers.

## Next steps
- See `api-reference.md` for core types.
---

| Prev | Next |
| --- | --- |
| [Installation](installation.md) | [Quickstart](quickstart.md) |
