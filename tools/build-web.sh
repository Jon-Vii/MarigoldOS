#!/usr/bin/env bash
# Build the browser emulator wasm for every supported board and stage the
# static site (web assets + one wasm per board) into OUT_DIR.
#
# The panel geometry is compile-time in the firmware, so each board is a
# separate wasm build. The crate always emits x4_web_emulator.wasm, so each
# board is built in turn and copied under a per-board name; index.html loads
# the right one from the ?board= query parameter (default x4).
#
# Usage: tools/build-web.sh [OUT_DIR]     (default: _site)
set -euo pipefail

OUT_DIR="${1:-_site}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$ROOT/tools/web-emulator/Cargo.toml"
WASM="$ROOT/tools/web-emulator/target/wasm32-unknown-unknown/release/x4_web_emulator.wasm"

mkdir -p "$OUT_DIR"
cp -R "$ROOT/web/." "$OUT_DIR/"

# Book bodies are runtime-fetched static assets, not compiled into the wasm;
# index.html's BOOK_FILES list matches books.rs's SHELF order.
mkdir -p "$OUT_DIR/books"
cp "$ROOT/tools/web-emulator/books/"*.txt "$OUT_DIR/books/"

build() {
  local board="$1"; shift
  cargo build --manifest-path "$MANIFEST" \
    --target wasm32-unknown-unknown --release "$@"
  local out="$OUT_DIR/${board}_web_emulator.wasm"
  cp "$WASM" "$out"
  # Best-effort size pass: CI installs binaryen (pages.yml), locally it is
  # optional — the un-optimized wasm is fully functional, just heavier.
  if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz --strip-debug --strip-producers "$out" -o "$out"
  else
    echo "wasm-opt not found; skipping size pass for ${board} (install binaryen to enable)" >&2
  fi
}

build x4
build x3 --features device-x3

echo "Wrote web assets and board wasm files to $OUT_DIR"
