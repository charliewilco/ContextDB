#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INPUT="${1:-${ROOT}/dist/ContextDB.xcframework}"
OUTPUT="${2:-${ROOT}/dist/ContextDB.xcframework.zip}"

if [[ ! -d "${INPUT}" ]]; then
	echo "XCFramework does not exist: ${INPUT}" >&2
	exit 1
fi

if [[ -e "${OUTPUT}" ]]; then
	echo "Output already exists: ${OUTPUT}" >&2
	echo "Move or remove it explicitly before rebuilding." >&2
	exit 1
fi

mkdir -p "$(dirname "${OUTPUT}")"
ditto -c -k --keepParent --norsrc --noextattr "${INPUT}" "${OUTPUT}"

echo "Created ${OUTPUT}"
echo "SwiftPM checksum: $(swift package compute-checksum "${OUTPUT}")"
