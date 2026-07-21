#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-${ROOT}/dist/ContextDB.xcframework}"
BUILD_DIR="${CARGO_TARGET_DIR:-${ROOT}/target}"
INTERMEDIATES="$(mktemp -d "${TMPDIR:-/tmp}/contextdb-xcframework.XXXXXX")"

cleanup() {
	rm -rf "${INTERMEDIATES}"
}
trap cleanup EXIT

export IPHONEOS_DEPLOYMENT_TARGET=15.0
export MACOSX_DEPLOYMENT_TARGET=12.0

if [[ -e "${OUTPUT}" ]]; then
	echo "Output already exists: ${OUTPUT}" >&2
	echo "Move or remove it explicitly before rebuilding." >&2
	exit 1
fi

rustup target add \
	aarch64-apple-ios \
	aarch64-apple-ios-sim \
	x86_64-apple-ios \
	aarch64-apple-darwin \
	x86_64-apple-darwin

for target in \
	aarch64-apple-ios \
	aarch64-apple-ios-sim \
	x86_64-apple-ios \
	aarch64-apple-darwin \
	x86_64-apple-darwin; do
	cargo build \
		--manifest-path "${ROOT}/Cargo.toml" \
		--release \
		--locked \
		--features ffi \
		--target "${target}"
done

mkdir -p "${INTERMEDIATES}/ios-simulator" "${INTERMEDIATES}/macos"
lipo -create \
	"${BUILD_DIR}/aarch64-apple-ios-sim/release/libcontextdb.a" \
	"${BUILD_DIR}/x86_64-apple-ios/release/libcontextdb.a" \
	-output "${INTERMEDIATES}/ios-simulator/libcontextdb.a"
lipo -create \
	"${BUILD_DIR}/aarch64-apple-darwin/release/libcontextdb.a" \
	"${BUILD_DIR}/x86_64-apple-darwin/release/libcontextdb.a" \
	-output "${INTERMEDIATES}/macos/libcontextdb.a"

mkdir -p "$(dirname "${OUTPUT}")"
xcodebuild -create-xcframework \
	-library "${BUILD_DIR}/aarch64-apple-ios/release/libcontextdb.a" \
	-headers "${ROOT}/include" \
	-library "${INTERMEDIATES}/ios-simulator/libcontextdb.a" \
	-headers "${ROOT}/include" \
	-library "${INTERMEDIATES}/macos/libcontextdb.a" \
	-headers "${ROOT}/include" \
	-output "${OUTPUT}"

echo "Created ${OUTPUT}"
