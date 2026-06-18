#!/usr/bin/env bash
# install.sh — build and install threshold to ~/.local/bin/threshold
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Building threshold (release)..."
cargo build --release

DEST="${HOME}/.local/bin/threshold"
mkdir -p "$(dirname "$DEST")"
cp target/release/threshold "$DEST"
chmod +x "$DEST"

echo "Installed: $DEST"
"$DEST" --help
