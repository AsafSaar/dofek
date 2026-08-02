#!/usr/bin/env bash
# Build the binaries Tauri ships as `externalBin` sidecars and copy each to the
# target-triple-suffixed name the sidecar mechanism expects.
#
# Tauri requires every sidecar to exist as
# `target/release/<name>-<triple><ext>` *before* the GUI build script runs,
# otherwise it fails with "resource path ... doesn't exist".
#
# Since v1.7 that is four binaries: dofek-tui plus the three first-party
# plugins. Bundling the plugins makes plugin install one click and offline —
# `resolve_command` probes the executable's directory for exactly these names
# (see `BUNDLED_PLUGINS` in src/plugin/store.rs) — and means they inherit the
# app's code signing and notarization for free rather than needing their own.
#
# Wired into gui/tauri.conf.json as beforeDevCommand / beforeBuildCommand so
# `cargo gui` and `cargo build-gui` prepare the sidecars automatically. It also
# fires during build-all.sh, since that calls `cargo tauri build`.
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
cargo build --release -p dofek-ollama -p dofek-docker -p dofek-net-ping

for NAME in dofek-tui dofek-ollama dofek-docker dofek-net-ping; do
    SRC="target/release/${NAME}${EXT}"
    DST="target/release/${NAME}-${TARGET_TRIPLE}${EXT}"
    if [ ! -f "$SRC" ]; then
        echo "prep-sidecar: expected binary not found: $SRC" >&2
        exit 1
    fi
    cp "$SRC" "$DST"
    echo "Sidecar ready: ${DST}"
done
