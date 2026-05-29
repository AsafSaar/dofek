#!/usr/bin/env bash
# Build the dofek-tui release binary and copy it to the target-triple-suffixed
# name that Tauri's `externalBin` sidecar mechanism expects.
#
# Tauri requires the sidecar binary to exist as
# `target/release/dofek-tui-<triple><ext>` *before* the GUI build script runs,
# otherwise it fails with "resource path ... doesn't exist".
#
# Wired into gui/tauri.conf.json as beforeDevCommand / beforeBuildCommand so
# `cargo gui` and `cargo build-gui` prepare the sidecar automatically.
# Mirrors the sidecar step in build-all.sh.
set -euo pipefail

# Run from the repo root regardless of where the hook invokes us.
cd "$(dirname "$0")/.."

# Detect the Rust target triple (e.g. aarch64-apple-darwin).
TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')

case "$TARGET_TRIPLE" in
    *windows*) EXT=".exe" ;;
    *)         EXT=""     ;;
esac

cargo build --release -p dofek --bin dofek-tui

SRC="target/release/dofek-tui${EXT}"
DST="target/release/dofek-tui-${TARGET_TRIPLE}${EXT}"
cp "$SRC" "$DST"
echo "Sidecar ready: ${DST}"
