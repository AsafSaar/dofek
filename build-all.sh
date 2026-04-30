#!/usr/bin/env bash
# Build both TUI and GUI, then package the platform-native installers.
# Windows host -> .msi (via WiX). Linux host -> .deb / .AppImage / .rpm.
# Usage: ./build-all.sh
set -euo pipefail

# Detect the Rust target triple
TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
echo "Target: $TARGET_TRIPLE"

case "$TARGET_TRIPLE" in
    *windows*) EXT=".exe" ;;
    *)         EXT=""     ;;
esac

echo ""
echo "=== Building dofek-tui (release) ==="
cargo build --release -p dofek --bin dofek-tui

# Tauri externalBin expects the binary name with the target triple appended
SRC="target/release/dofek-tui${EXT}"
DST="target/release/dofek-tui-${TARGET_TRIPLE}${EXT}"
echo "Copying ${SRC} → ${DST}"
cp "$SRC" "$DST"

echo ""
echo "=== Building first-party plugins (release) ==="
cargo build --release -p dofek-ollama -p dofek-docker -p dofek-net-ping
echo "Plugin binaries:"
echo "  target/release/dofek-ollama${EXT}"
echo "  target/release/dofek-docker${EXT}"
echo "  target/release/dofek-net-ping${EXT}"

echo ""
echo "=== Building dofek-gui + native bundles ==="
cd gui
cargo tauri build

echo ""
echo "=== Done ==="
echo "Bundles in: target/release/bundle/"
echo "Plugin binaries (ship as optional add-ons): target/release/dofek-{ollama,docker,net-ping}${EXT}"
