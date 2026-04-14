#!/usr/bin/env bash
# Build both TUI and GUI, then package into a single MSI installer.
# Usage: ./build-all.sh
set -euo pipefail

# Detect the Rust target triple
TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
echo "Target: $TARGET_TRIPLE"

echo ""
echo "=== Building dofek-tui (release) ==="
cargo build --release -p dofek --bin dofek-tui

# Tauri externalBin expects the binary name with the target triple appended
echo "Copying dofek-tui.exe → dofek-tui-${TARGET_TRIPLE}.exe"
cp "target/release/dofek-tui.exe" "target/release/dofek-tui-${TARGET_TRIPLE}.exe"

echo ""
echo "=== Building dofek-gui + MSI bundle ==="
cd gui
cargo tauri build

echo ""
echo "=== Done ==="
echo "MSI installer: target/release/bundle/msi/"
