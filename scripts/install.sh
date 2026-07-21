#!/usr/bin/env bash

set -euo pipefail

REPOSITORY="https://github.com/charliewilco/contextdb"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
	echo "Install the pre-alpha ContextDB CLI from the current main branch."
	echo "Usage: install.sh"
	exit 0
fi

if [[ $# -gt 0 ]]; then
	echo "Unknown option: $1" >&2
	exit 2
fi

if ! command -v cargo >/dev/null 2>&1; then
	echo "Rust and Cargo are required. Install them from https://rustup.rs." >&2
	exit 1
fi

echo "Installing the pre-alpha ContextDB CLI from ${REPOSITORY}..."
cargo install \
	--git "${REPOSITORY}" \
	--branch main \
	--locked \
	--features cli \
	--bin contextdb

echo "Installed ContextDB. Run: contextdb --help"
