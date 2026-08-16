#!/usr/bin/env bash
# Create empty stand-ins for the binaries Tauri ships as `externalBin` sidecars,
# so `cargo clippy --workspace` / `cargo test --workspace` can compile the
# dofek-gui crate in CI.
#
# Why this exists: Tauri's build script resolves every `externalBin` path at
# *compile* time and aborts with "resource path ... doesn't exist" if one is
# missing. Locally that is satisfied by gui/prep-sidecar.{sh,ps1}, which the
# tauri.conf.json beforeBuildCommand hook runs — but those do a full release
# build of four binaries, which is minutes per runner that a lint-and-test job
# has no reason to spend. CI never bundles, so nothing ever reads these files;
# only their existence is checked.
#
# The name list is duplicated from gui/prep-sidecar.sh, which is the real one.
# If the two drift, the gui build fails loudly in CI with Tauri's own error
# naming the missing path — which is the outcome we want from a mismatch.
set -euo pipefail

TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
case "$TARGET_TRIPLE" in
    *windows*) EXT=".exe" ;;
    *)         EXT=""     ;;
esac

mkdir -p target/release
for NAME in dofek-tui dofek-ollama dofek-docker dofek-net-ping; do
    DST="target/release/${NAME}-${TARGET_TRIPLE}${EXT}"
    # Never clobber a real binary a previous step (or a warm cache) left behind.
    if [ -s "$DST" ]; then
        echo "Sidecar already present, leaving it alone: ${DST}"
    else
        : > "$DST"
        echo "Placeholder sidecar: ${DST}"
    fi
done
