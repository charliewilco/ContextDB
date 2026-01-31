#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGETS=(
  "aarch64-apple-ios"
  "aarch64-apple-ios-sim"
  "x86_64-apple-ios"
  "aarch64-apple-darwin"
  "x86_64-apple-darwin"
)

FEATURES="ffi"
LIB_NAME="contextdb"
HEADER_DIR="$ROOT_DIR/include"
OUTPUT_DIR="$ROOT_DIR/dist/ContextDB.xcframework"

cd "$ROOT_DIR"

mkdir -p "$ROOT_DIR/dist"

for target in "${TARGETS[@]}"; do
  cargo build --release --lib --features "$FEATURES" --target "$target"
  if [[ ! -f "target/$target/release/lib${LIB_NAME}.a" ]]; then
    echo "Expected static library missing for $target" >&2
    exit 1
  fi
  echo "Built $target"
done

echo "Packaging XCFramework..."
rm -rf "$OUTPUT_DIR"

xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/release/lib${LIB_NAME}.a" -headers "$HEADER_DIR" \
  -library "target/aarch64-apple-ios-sim/release/lib${LIB_NAME}.a" -headers "$HEADER_DIR" \
  -library "target/x86_64-apple-ios/release/lib${LIB_NAME}.a" -headers "$HEADER_DIR" \
  -library "target/aarch64-apple-darwin/release/lib${LIB_NAME}.a" -headers "$HEADER_DIR" \
  -library "target/x86_64-apple-darwin/release/lib${LIB_NAME}.a" -headers "$HEADER_DIR" \
  -output "$OUTPUT_DIR"

echo "XCFramework written to $OUTPUT_DIR"
