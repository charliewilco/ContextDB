# Swift Package Distribution

## Overview
ContextDB ships a Swift Package that wraps the Rust static library via an XCFramework.
The package is binary-based so you can import `ContextDB` directly in Swift.

## Build the XCFramework
Install the required Rust targets:

```sh
rustup target add \
  aarch64-apple-ios \
  aarch64-apple-ios-sim \
  x86_64-apple-ios \
  aarch64-apple-darwin \
  x86_64-apple-darwin
```

Build the XCFramework for SwiftPM:

```sh
scripts/build_spm_xcframework.sh
```

This writes `dist/ContextDB.xcframework`.

## Use the Swift Package (local)
In Xcode, add the repo as a **local package** and select the `ContextDB` product.
You can also add it in a `Package.swift` dependency:

```swift
.package(path: "/path/to/ContextDB")
```

Then:

```swift
import ContextDB
```

## Publishing
If you want to publish a remote Swift Package:
1. Zip `dist/ContextDB.xcframework`.
2. Upload the zip to a GitHub release.
3. Update `Package.swift` to use a `binaryTarget(url:checksum:)` with the release URL.

## Notes
- The module map lives in `include/module.modulemap`.
- The FFI header is `include/contextdb.h`.
