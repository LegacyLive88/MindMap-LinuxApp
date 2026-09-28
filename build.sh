#!/usr/bin/env bash
# Compile MindMap into one program. Maps are stored beside that program.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release
mkdir -p dist
cp -f target/release/mindmap dist/mindmap
chmod +x dist/mindmap
echo "Built dist/mindmap"
echo "Run ./dist/mindmap — canvases are saved in dist/mindmap-data/."
