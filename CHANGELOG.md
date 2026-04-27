# Changelog

All notable changes to dofek are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.3.0] - 2026-04-27

System-tray companion is here — dofek's OS chrome itself is now a system monitor — alongside Linux CPU power via RAPL and cross-platform disk I/O metrics.

### Added
- **System-tray / menu-bar companion** (Windows, Linux, macOS). Live-rendered 32×32 RGBA icon with a CPU sparkline that ramps sky-blue → emerald → amber → red as load climbs. Right-click menu (Show / Hide / Settings… / About / Quit). Left-click toggles the main window.
- **Close-to-tray default**: pressing the window close button now hides to the tray instead of quitting; quit only via tray menu, `Cmd+Q`, or `Alt+F4`. Configurable in the help/settings overlay.
- **Start-in-tray** option: launches with the main window hidden; useful for autostart.
- **macOS menu-bar text** (`CPU NN GPU NN`) next to the tray icon when `tray_show_text=true`. Toggleable.
- **Linux CPU power via RAPL** — reads `/sys/class/powercap/intel-rapl:0/energy_uj` and exposes watts in the `cpu.power` field. Self-disables silently on permission denied or non-Intel hosts.
- **Disk I/O metrics** — new cross-platform tracker (`sysinfo::Disks`) producing aggregate read/write rates plus a per-device list. New `DISK` chart tab (TUI key `d`, GUI tab) with stacked area chart for read/write. New ticker pill (gated to appear only when aggregate I/O > 1 KB/s).
- **`dofek-tui` ticker self-syncs to `Cargo.toml`** — the version literal is now `concat!(" v", env!("CARGO_PKG_VERSION"))` so future bumps need one fewer manual edit. Same change applied to the TUI About overlay.
- **Tauri single-instance plugin** on Windows/Linux — second invocations now just focus the existing window instead of opening a duplicate (and a duplicate tray icon).
- **Three new telemetry events**: `tray_icon_clicked`, `tray_menu_item_selected { item }`, `window_closed_to_tray`.
- **Four new persisted settings**: `enable_tray`, `close_to_tray`, `start_in_tray`, `tray_show_text`. Old `settings.toml` files load through unchanged via `#[serde(default)]`.

### Changed
- Workspace version bumped to 1.3.0 across `dofek`, `dofek-gui`, `gui/tauri.conf.json`, README, install notes, manual, and website.
- Snapshot relay thread now lives inside the Tauri `setup` closure so the tray can be re-rendered from the same loop that updates shared state — no second collector, no duplicate IPC.
- Disk theme color reassigned to amber (`#EAB308`) to distinguish from CPU sky-blue and NET orange in the new chart.

### Notes
- **GNOME tray:** GNOME removed legacy tray support. The tray icon needs the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/); without it the tray won't appear (other features unaffected). Documented; not worked around.
- **AMD CPU power on Linux:** `intel-rapl` covers Intel and some AMD parts (via `intel_rapl_common`). Pure `amd_energy` paths and `CAP_SYS_RAWIO` workarounds are tracked for v1.4.
- **macOS:** GPU/VRAM and CPU temp/power are still N/A — Apple Silicon SMC integration tracked for a future release.
- **Downgrading:** a v1.3 user with `chart_tab = "disk"` opening on v1.2 silently resets to `cpu`. No data loss, no crash.

## [1.2.0] - 2026-04-26

macOS support — dofek now runs natively on Apple Silicon Macs (`aarch64-apple-darwin`).

### Added
- **macOS (Apple Silicon) support** for both the TUI (`dofek-tui`) and the Tauri GUI (`dofek-gui`).
- Unsigned `.app` bundle for the GUI and standalone TUI binary, distributed via the Releases page (the `.app` is zipped to preserve its directory structure on download).
- macOS arm of `os_version_string()` parses `sw_vers` to produce strings like `macOS 14.6 (Sonoma)`, with codename mapping for Big Sur through Sequoia.
- macOS-specific synthetic-interface filter in the network module (skips `awdl0`, `llw0`, `gif0`, `stf0`, `anpi0`/`1`, `ap1`); `lo0` is now also treated as loopback alongside `lo`.
- `check-macos` job in CI on `macos-latest`, alongside the existing Windows and Linux matrices.
- `build-macos` job in the release pipeline, producing `dofek-tui` and `dofek-gui-aarch64-apple-darwin.app.zip` with `shasum` checksums; the combined release `SHA256SUMS.txt` now spans all three platforms.

### Changed
- Workspace version bumped to 1.2.0 across `dofek`, `dofek-gui`, and `gui/tauri.conf.json`.
- README, CLAUDE.md, and release notes updated for the three-platform story; config-directory docs now include `~/Library/Application Support/dofek/` for macOS.

### Notes
- **macOS limitations (v1.2):** GPU/VRAM and CPU temperature/power are not implemented; those panels show N/A. NVML is NVIDIA-only, and Apple Silicon SMC sensor coverage in `sysinfo` is not yet sufficient. Tracked for a future release.
- **Apple Silicon only.** Intel Macs are not supported; no universal binary in v1.
- macOS binaries are unsigned. Gatekeeper will block first launch — right-click → **Open**, or run `xattr -dr com.apple.quarantine /Applications/dofek.app` once.
- AMD GPU VRAM, CPU power on Linux (RAPL), and Linux ARM64 remain on the roadmap.

## [1.1.0] - 2026-04-25

Linux support — dofek is now a first-class Linux application alongside Windows.

### Added
- **Linux x86_64 support** for both the TUI (`dofek-tui`) and the Tauri GUI (`dofek-gui`).
- Native Linux installers / packages: `.deb`, `.AppImage`, `.rpm`, alongside the existing Windows `.msi`.
- CPU temperature on Linux via `sysinfo::Components` (reads `/sys/class/hwmon`) — no LibreHardwareMonitor dependency.
- Network statistics on Linux via `sysinfo::Networks`, sharing the same rate-tracking machinery as the Windows `GetIfTable2` path.
- Process kill on Linux via `nix::sys::signal::kill(SIGTERM)`.
- Cross-platform local time rendering via `chrono::Local` (replaces both the Windows `GetLocalTime` and the previous UTC fallback).
- `check-linux` job in CI on `ubuntu-latest`, mirroring the existing Windows lint+test job.
- `build-linux` job in the release pipeline, producing `dofek-tui`, `.deb`, `.rpm`, and `.AppImage` with `sha256sum` checksums.

### Changed
- Config and settings now look up `dirs::config_dir()` (Windows: `%APPDATA%\dofek`, Linux: `~/.config/dofek`).
- Hostname now comes from `sysinfo::System::host_name()` instead of the `COMPUTERNAME` env var.
- OS version reporting renamed `windows_version_string()` → `os_version_string()`; the Linux branch reads `/etc/os-release` `PRETTY_NAME`.
- Tauri bundle config switched to `targets: "all"` so each runner produces its native bundles.
- README, CLAUDE.md, and `dofek.toml.example` updated for the dual-OS story.

### Notes
- AMD GPU VRAM, CPU power on Linux (RAPL), macOS, and ARM64 remain on the roadmap.
- The Linux GUI build requires Tauri's standard apt deps: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`, `libgtk-3-dev` (and `rpm` for `.rpm` packaging).

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
