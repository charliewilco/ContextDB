#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "scripts/ios_build.sh is deprecated; using scripts/build_xcframework.sh" >&2
exec "${ROOT}/scripts/build_xcframework.sh" "$@"
