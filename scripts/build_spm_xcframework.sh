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
MACOS_UNIVERSAL_DIR="$ROOT_DIR/target/universal-macos"
MACOS_UNIVERSAL_LIB="$MACOS_UNIVERSAL_DIR/lib${LIB_NAME}.a"
IOS_SIM_UNIVERSAL_DIR="$ROOT_DIR/target/universal-ios-sim"
IOS_SIM_UNIVERSAL_LIB="$IOS_SIM_UNIVERSAL_DIR/lib${LIB_NAME}.a"

cd "$ROOT_DIR"

mkdir -p "$ROOT_DIR/dist"
mkdir -p "$MACOS_UNIVERSAL_DIR"
mkdir -p "$IOS_SIM_UNIVERSAL_DIR"

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

echo "Creating macOS universal static library..."
lipo -create \
  "target/aarch64-apple-darwin/release/lib${LIB_NAME}.a" \
  "target/x86_64-apple-darwin/release/lib${LIB_NAME}.a" \
  -output "$MACOS_UNIVERSAL_LIB"

echo "Creating iOS simulator universal static library..."
lipo -create \
  "target/aarch64-apple-ios-sim/release/lib${LIB_NAME}.a" \
  "target/x86_64-apple-ios/release/lib${LIB_NAME}.a" \
  -output "$IOS_SIM_UNIVERSAL_LIB"

xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/release/lib${LIB_NAME}.a" -headers "$HEADER_DIR" \
  -library "$IOS_SIM_UNIVERSAL_LIB" -headers "$HEADER_DIR" \
  -library "$MACOS_UNIVERSAL_LIB" -headers "$HEADER_DIR" \
  -output "$OUTPUT_DIR"

echo "XCFramework written to $OUTPUT_DIR"
