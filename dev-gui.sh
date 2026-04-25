#!/usr/bin/env bash
# dev-gui.sh — Tauri dev server with the externalBin TUI binary staged.
#
# `cargo tauri dev` (the alias `cargo gui`) refuses to start until the
# externalBin path declared in gui/tauri.conf.json exists, suffixed with the
# host target triple. This script builds dofek-tui in release mode, copies it
# with the right name, then runs the Tauri dev server.
#
# Usage: ./dev-gui.sh

set -euo pipefail

cd "$(dirname "$0")"

TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
case "$TARGET_TRIPLE" in
    *windows*) EXT=".exe" ;;
    *)         EXT=""     ;;
esac

echo "=== Building dofek-tui (release) ==="
cargo build --release -p dofek --bin dofek-tui

SRC="target/release/dofek-tui${EXT}"
DST="target/release/dofek-tui-${TARGET_TRIPLE}${EXT}"
echo "Staging ${SRC} → ${DST}"
cp -f "$SRC" "$DST"

echo "=== Starting Tauri dev server ==="
cd gui
exec cargo tauri dev
