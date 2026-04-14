# Install a Windows Terminal profile for dofek TUI with optimal font settings.
# Usage: .\install-wt-profile.ps1
# This adds a "dofek" profile to Windows Terminal with font size 9 and JetBrains Mono.

$ErrorActionPreference = "Stop"

# Find Windows Terminal settings.json
$wtPaths = @(
    "$env:LOCALAPPDATA\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json",
    "$env:LOCALAPPDATA\Microsoft\Windows Terminal\settings.json"
)
$settingsPath = $wtPaths | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $settingsPath) {
    Write-Host "Windows Terminal settings not found. Is Windows Terminal installed?" -ForegroundColor Red
    exit 1
}

# Find dofek-tui.exe — check PATH first, then common locations
$tuiExe = Get-Command dofek-tui.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $tuiExe) {
    $candidates = @(
        "$PSScriptRoot\target\release\dofek-tui.exe",
        "$env:ProgramFiles\dofek\dofek-tui.exe",
        "$env:LOCALAPPDATA\dofek\dofek-tui.exe"
    )
    $tuiExe = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $tuiExe) {
    Write-Host "dofek-tui.exe not found in PATH or common locations." -ForegroundColor Red
    Write-Host "Build it first: cargo build-tui" -ForegroundColor Yellow
    exit 1
}

Write-Host "Found: $tuiExe"
Write-Host "Settings: $settingsPath"

# Read current settings
$settings = Get-Content $settingsPath -Raw | ConvertFrom-Json

# Check if profile already exists
$existing = $settings.profiles.list | Where-Object { $_.name -eq "dofek" }
if ($existing) {
    Write-Host "Profile 'dofek' already exists in Windows Terminal. Updating..." -ForegroundColor Yellow
    $existing.commandline = $tuiExe
    $existing.font = @{ face = "JetBrains Mono"; size = 9 }
} else {
    # Create new profile
    $profile = [PSCustomObject]@{
        name        = "dofek"
        commandline = $tuiExe
        icon        = $null
        font        = @{ face = "JetBrains Mono"; size = 9 }
        colorScheme = "One Half Dark"
        cursorShape = "vintage"
        hidden      = $false
    }
    $settings.profiles.list += $profile
    Write-Host "Added 'dofek' profile to Windows Terminal." -ForegroundColor Green
}

# Write back
$settings | ConvertTo-Json -Depth 10 | Set-Content $settingsPath -Encoding UTF8
Write-Host "Done! Open Windows Terminal and select the 'dofek' profile from the dropdown." -ForegroundColor Green
