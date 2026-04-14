# Build both TUI and GUI, then package into a single MSI installer.
# Usage: .\build-all.ps1
$ErrorActionPreference = "Stop"

# Detect the Rust target triple
$triple = (rustc -vV | Select-String "^host:").ToString().Split(" ")[1]
Write-Host "Target: $triple"

Write-Host ""
Write-Host "=== Building dofek-tui (release) ==="
cargo build --release -p dofek --bin dofek-tui
if ($LASTEXITCODE -ne 0) { exit 1 }

# Tauri externalBin expects the binary name with the target triple appended
Write-Host "Copying dofek-tui.exe -> dofek-tui-$triple.exe"
Copy-Item "target\release\dofek-tui.exe" "target\release\dofek-tui-$triple.exe" -Force

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
