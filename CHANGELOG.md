# Changelog

All notable changes to dofek are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-04-23

First public, generally-available release.

### Added
- Public GitHub repository, MSI installer, and direct download links from [dofek.dev](https://dofek.dev)
- `LICENSE` (MIT), `CHANGELOG.md`, `CONTRIBUTING.md`, and `SECURITY.md` at the repository root
- `.github/workflows/release.yml` — automated tag-driven build of TUI binary, GUI installer, and `SHA256SUMS.txt`
- `.github/workflows/ci.yml` — clippy + tests on every PR and push to `main`
- Issue templates for bug reports and feature requests
- Real TUI/GUI screenshots embedded in the README
- **Offline user manual** bundled with the MSI (`manual.html`) — accessible from the Start Menu ("dofek Manual") and from the GUI help overlay ("Open full manual" button)
- `README.txt` in the install directory as a quick pointer to the manual and config locations

### Changed
- Plugin API explicitly marked as **experimental** until further notice; the `schema_version: 1` field allows plugins to pin against breaking changes
- README restructured with badges, downloads section, and clearer install path

### Notes
- Binaries are unsigned for v1.0. Code signing is on the post-1.0 roadmap. Windows SmartScreen may prompt on first run.

## [0.8.0] - prior

Centered loading state, Ollama plugin, GUI icon, Windows Terminal profile icon.

## [0.7.0] - prior

Process tree / grouped view, expanded LibreHardwareMonitor integration, GUI process management.

## [0.6.0] - prior

Process management (search, kill, kill-all), interactive process table, LHM CPU temp/power.

## [0.5.0] - prior

Telemetry settings persistence, GUI help modal improvements.

## [0.4.0] - prior

Performance optimizations, GUI polish, MSI installer, cargo aliases, SEO.

## [0.3.0] - prior

Plugin system (JSON-over-stdio protocol), `dofek-ollama` and `dofek-docker` plugins.

## [0.2.0] - prior

Trading-terminal layout, candlestick charts, multi-GPU, process categories, Tauri GUI, resizable panes.

## [0.1.0] - prior

Initial proof-of-concept: terminal-native system monitor.
