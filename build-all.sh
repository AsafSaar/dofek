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
echo "=== Building dofek-tui + first-party plugins (release) ==="
# Delegated rather than repeated here: since v1.7 there are four externalBin
# sidecars (dofek-tui plus the three plugins), and prep-sidecar.sh is the one
# place that list lives. This script used to build the TUI and stage only its
# suffixed copy, which silently covered one sidecar out of four.
#
# `cargo tauri build` below re-runs this via the beforeBuildCommand hook, which
# is a no-op second time round. Running it up front keeps a build failure in a
# plugin crate from surfacing as an opaque Tauri hook error.
./gui/prep-sidecar.sh

echo ""
echo "=== Building dofek-gui + native bundles ==="
cd gui
cargo tauri build

echo ""
echo "=== Done ==="
echo "Bundles in: target/release/bundle/"
echo "Plugin binaries (ship as optional add-ons): target/release/dofek-{ollama,docker,net-ping}${EXT}"
