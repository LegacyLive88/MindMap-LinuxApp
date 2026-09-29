#!/usr/bin/env bash
# Cross-compile the standalone Windows 64-bit MindMap program into this folder.
# Requires Rust and the MinGW gcc (package gcc-mingw-w64-x86-64 on Debian/Ubuntu).
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v x86_64-w64-mingw32-gcc-posix >/dev/null 2>&1; then
  echo "x86_64-w64-mingw32-gcc-posix was not found. Install gcc-mingw-w64-x86-64." >&2
  exit 1
fi

rustup target add x86_64-pc-windows-gnu

# The posix thread model matches Rust's Windows GNU standard library.
# crt-static links libgcc and winpthreads into the exe so it needs no extra DLLs.
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
export RUSTFLAGS="-C target-feature=+crt-static ${RUSTFLAGS:-}"

cargo build --release --target x86_64-pc-windows-gnu
cp -f target/x86_64-pc-windows-gnu/release/mindmap.exe win/mindmap.exe
echo "Built win/mindmap.exe"
echo "On Windows, canvases are saved in win/mindmap-data/ beside the program."
