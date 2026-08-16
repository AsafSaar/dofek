# Build both TUI and GUI, then package into a single MSI installer.
# Usage: .\build-all.ps1
$ErrorActionPreference = "Stop"

# Detect the Rust target triple
$triple = (rustc -vV | Select-String "^host:").ToString().Split(" ")[1]
Write-Host "Target: $triple"

Write-Host ""
Write-Host "=== Building dofek-tui + first-party plugins (release) ==="
# Delegated rather than repeated here: since v1.7 there are four externalBin
# sidecars (dofek-tui plus the three plugins), and prep-sidecar.ps1 is the one
# place that list lives. This script used to build the TUI and stage only its
# suffixed copy, which silently covered one sidecar out of four.
#
# `cargo tauri build` below re-runs this via the beforeBuildCommand hook, which
# is a no-op second time round. Running it up front keeps a build failure in a
# plugin crate from surfacing as an opaque Tauri hook error.
#
# Push/Pop-Location because prep-sidecar.ps1 anchors itself to the repo root
# with Set-Location, which would otherwise persist into this script and change
# how the relative paths below resolve.
Push-Location
try {
    & (Join-Path $PSScriptRoot "gui\prep-sidecar.ps1")
    if ($LASTEXITCODE -ne 0) { exit 1 }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "=== Building dofek-gui + MSI bundle ==="
Push-Location gui
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) { exit 1 }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "=== Done ==="
Write-Host "MSI installer: target\release\bundle\msi\"
Write-Host "Plugin binaries (ship as optional add-ons): target\release\dofek-ollama.exe, target\release\dofek-docker.exe, target\release\dofek-net-ping.exe"
