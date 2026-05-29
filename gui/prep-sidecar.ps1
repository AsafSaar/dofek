# Build the dofek-tui release binary and copy it to the target-triple-suffixed
# name that Tauri's `externalBin` sidecar mechanism expects.
#
# Tauri requires the sidecar binary to exist as
# target\release\dofek-tui-<triple>.exe *before* the GUI build script runs,
# otherwise it fails with "resource path ... doesn't exist".
#
# Wired into gui/tauri.conf.json as the Windows beforeDevCommand /
# beforeBuildCommand (via tauri.windows.conf.json) so `cargo gui` and
# `cargo build-gui` prepare the sidecar automatically. Mirrors build-all.ps1.
$ErrorActionPreference = "Stop"

# Run from the repo root regardless of where the hook invokes us.
Set-Location (Join-Path $PSScriptRoot "..")

# Detect the Rust target triple (e.g. x86_64-pc-windows-msvc).
$triple = (rustc -vV | Select-String "^host:").ToString().Split(" ")[1]

cargo build --release -p dofek --bin dofek-tui

$src = "target\release\dofek-tui.exe"
$dst = "target\release\dofek-tui-$triple.exe"
Copy-Item $src $dst -Force
Write-Host "Sidecar ready: $dst"
