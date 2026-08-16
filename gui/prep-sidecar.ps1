# Build the binaries Tauri ships as `externalBin` sidecars and copy each to the
# target-triple-suffixed name the sidecar mechanism expects.
#
# Tauri requires every sidecar to exist as
# target\release\<name>-<triple>.exe *before* the GUI build script runs,
# otherwise it fails with "resource path ... doesn't exist".
#
# Since v1.7 that is four binaries: dofek-tui plus the three first-party
# plugins. Bundling the plugins makes plugin install one click and offline —
# resolve_command probes the executable's directory for exactly these names
# (see BUNDLED_PLUGINS in src/plugin/store.rs) — and means they inherit the
# app's code signing for free rather than needing their own.
#
# Wired into gui/tauri.conf.json as the Windows beforeDevCommand /
# beforeBuildCommand (via tauri.windows.conf.json) so `cargo gui` and
# `cargo build-gui` prepare the sidecars automatically. Mirrors build-all.ps1.
$ErrorActionPreference = "Stop"

# Run from the repo root regardless of where the hook invokes us.
Set-Location (Join-Path $PSScriptRoot "..")

# Detect the Rust target triple (e.g. x86_64-pc-windows-msvc).
$triple = (rustc -vV | Select-String "^host:").ToString().Split(" ")[1]

cargo build --release -p dofek --bin dofek-tui
if ($LASTEXITCODE -ne 0) { exit 1 }
cargo build --release -p dofek-ollama -p dofek-docker -p dofek-net-ping
if ($LASTEXITCODE -ne 0) { exit 1 }

foreach ($name in @("dofek-tui", "dofek-ollama", "dofek-docker", "dofek-net-ping")) {
    $src = "target\release\$name.exe"
    $dst = "target\release\$name-$triple.exe"
    if (-not (Test-Path $src)) {
        Write-Error "prep-sidecar: expected binary not found: $src"
        exit 1
    }
    Copy-Item $src $dst -Force
    Write-Host "Sidecar ready: $dst"
}
