# Changelog

All notable changes to Dofek are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.0] - 2026-04-30

Minor release — turns the plugin system from a hand-rolled "edit dofek.toml + put a binary on PATH" workflow into a managed install experience driven from the GUI or CLI, with hot reload.

### Added
- **Managed plugin store.** New `<config_dir>/dofek/plugins/` directory holds installed binaries; new managed `plugins.toml` (separate from the user-owned `dofek.toml`) tracks `[[plugins]]` entries. `PluginStore` (`src/plugin/store.rs`) handles install (copy → `chmod +x` → clear `com.apple.quarantine` xattr on macOS → probe `manifest` via a single poll → register), remove, and enable/disable.
- **`dofek-tui plugins` CLI subcommand** (`src/plugin/cli.rs`) with `list`, `add <path> [-- <plugin args>]`, `remove`, `enable`, `disable`. The default `dofek-tui` invocation still launches the TUI; subcommands print to stdout and exit before the alt-screen takeover.
- **GUI plugin manager.** Settings overlay (`?`) gains a Plugins section with a native file picker (via `tauri-plugin-dialog`), an enable/disable toggle, and an uninstall button per row. New Tauri commands `plugins_list`, `plugins_add`, `plugins_remove`, `plugins_set_enabled`, `plugins_pick_file` wrap `PluginStore` directly so install via GUI is byte-equivalent to install via CLI.
- **Hot reload.** Data collector watches `plugins.toml` mtime each tick; on change, `PluginManager::replace()` shuts the old children down, kills stragglers after 200 ms, and rebuilds from the fresh config. Installs/removes/toggles take effect within one snapshot — no restart needed.
- **Plugin path resolver.** `PluginProcess::spawn` now resolves bare command names against the managed plugin directory before falling back to `PATH`, so installed plugins work without any environment changes.
- **`dofek-plugin-protocol` workspace crate** (`crates/dofek-plugin-protocol/`). Canonical serde types for the JSON-over-stdio protocol, with both `Serialize` and `Deserialize` so dofek and external plugin authors share definitions. Both first-party plugins now depend on it instead of redeclaring 50-line skeletons.
- **Per-plugin READMEs** for `dofek-ollama` and `dofek-docker`: prerequisites, build/install per OS, copy-paste `dofek.toml` snippet, CLI flag table, troubleshooting, security note on the Docker TCP socket.
- **Two-pane Settings dialog.** `?` overlay redesigned: Shortcuts on the left (with the Open full manual button anchored at the bottom), Settings + Plugins on the right (right pane scrolls independently). Falls back to single-column below 720px viewport. Status-bar label `? help` → `? settings` in both TUI and GUI.

### Changed
- **`Config::load` no longer merges managed plugins.** `Config::plugins` now contains only user-owned `dofek.toml` entries; the data collector composes `[user + managed]` at startup so it can re-merge cleanly when plugins.toml changes. User-edited `dofek.toml` is never round-tripped through serde.
- **`build-all.sh` / `build-all.ps1`** now also build `dofek-ollama` and `dofek-docker` in release mode and stage their binaries next to the installer artefacts (still optional add-ons, not bundled into the MSI/deb/rpm yet).
- **`dofek.toml.example`** plugin block now documents the managed-install path; first-party plugin entries remain commented-out templates.

### Notes
- Plugin trust model is unchanged: an installed plugin runs as a child process with the same privileges as Dofek. The macOS quarantine clear is the same `xattr -d com.apple.quarantine` users do manually today — automated only for files they explicitly chose to install.
- A curated registry / Browse tab for one-click install of official plugins (Phase 2 from the v1.5 plan) is not in this release; tracked for v1.6+.

## [1.4.0] - 2026-04-29

Minor release — adds a notify-only update checker, a 3-mode tray display selector, and capitalizes the brand name in user-facing surfaces.

### Added
- **Check for updates** across both interfaces. New `src/update.rs` queries the GitHub Releases API and compares against the compiled-in version with an inline semver compare (no new crate, reuses existing `ureq`). TUI: `u` key opens an overlay (current vs latest, release URL, first lines of notes). GUI: "Check now" button in settings; topbar version pill turns into a clickable update link when a newer release exists. Notify-only — never downloads or installs anything. New `check_updates_on_startup` setting (default off) runs a silent probe at launch and surfaces the overlay/toast only when a newer release lands.
- **Tray display selector** with three modes: chart only, chart + text, text only (macOS). Replaces the previous binary "Menu-bar text" toggle; legacy `tray_show_text` is kept as a fallback so 1.3.x users don't lose their previous choice. Mode changes apply immediately — `save_settings` calls `tray::update` synchronously instead of waiting for the next data-collector tick.

### Changed
- **Brand display name capitalized to "Dofek"** across all user-facing surfaces: TUI overlays/ticker/snapshot text/CLI warnings, GUI window title/logo/About/Help/tray menu/tooltips/toasts, Tauri `productName`, AppStream `<name>`, capabilities description, plugin-manifest `author`, README/SECURITY/CONTRIBUTING/manual prose, and the `dofek.dev` website. Every identifier (crate names, binary names, bundle ID `com.dofek.app`, desktop ID, repo URL, domain, config filename `dofek.toml`, config dirs, snapshot dir, Tauri event scheme `dofek://`) is intentionally unchanged so existing installs and `cargo install` flows are unaffected.
- **Bundle artifact filenames change as a side effect of the `productName` flip.** Tauri 2 derives `.app`, `.dmg`, and `.msi` names from `productName`, so v1.4+ artifacts ship as `Dofek.app` / `Dofek_*.dmg` / `Dofek_*.msi` (was `dofek_*`). The v1.3.x release artifacts on GitHub keep their original names. README and bundled manual now reference the new `Dofek.app` Gatekeeper string and `xattr -dr com.apple.quarantine /Applications/Dofek.app` fix.
- **Tray icon for text-only mode** uses `set_icon(None)` instead of a transparent 32×32 pixmap. The transparent-pixmap path left a phantom indent before the title on macOS NSStatusItem; passing `None` actually drops the image slot. Title clearing for chart-only mode now passes an empty string instead of `None` to `set_title` — Tauri's bridge does not reliably clear NSStatusItem titles when given `None`.

### Notes
- macOS GPU/VRAM and CPU temp/power remain N/A; tracked for a future release.
- Code signing is still pending — Gatekeeper "damaged" workaround still required on first launch.

## [1.3.4] - 2026-04-29

Patch release — TUI refresh-rate config now actually controls the data-collection cadence, and the macOS Gatekeeper "damaged" workaround is documented prominently.

### Fixed
- **TUI `refresh_ms` from `dofek.toml` is now the source of truth.** Two layered bugs were stacking: the chart top-right legend hardcoded `"500ms"` regardless of the configured rate, and `apply_settings()` overwrote `App.refresh_ms` with `settings.toml`'s default (500), silently shadowing whatever the user set in `dofek.toml`. Both fixed; the legend now reflects the live cadence and `dofek.toml` wins for the persisted value.
- **TUI `+`/`-` keys now actually change the data-collection cadence.** Previously `+`/`-` updated the displayed number but the collector thread had captured `config.general.refresh_ms` at spawn time and ignored runtime changes — the chart legend would show "5000ms" while the chart kept updating at the original rate. The poll interval is now an `Arc<AtomicU64>` shared between `App` and the collector; runtime changes propagate on the next sleep iteration without respawning the thread. GUI keeps its 1 Hz floor for performance.

### Changed
- **macOS install docs surface the "damaged" Gatekeeper error prominently.** A dedicated callout under the macOS download table in `README.md` quotes the exact `"dofek.app is damaged and can't be opened"` string so users searching for it land on the `xattr -dr com.apple.quarantine` fix. The right-click → Open trick has been demoted to a Sonoma-and-earlier qualifier — macOS 15 Sequoia removed that bypass, so `xattr` is now the only working fix on current macOS until the app is signed and notarized (tracked for v1.4). README-install.txt, `docs/manual.html`, and the website install step were updated to match.

### Notes
- macOS GPU/VRAM and CPU temp/power remain N/A; tracked for a future release.

## [1.3.3] - 2026-04-28

Patch release — Linux WebKitGTK GUI no longer pegs CPU at idle, and the tray menu renders reliably on GNOME again. (1.3.2 skipped — the bump landed in the same commit as the fix.)

### Fixed
- **Linux WebKitGTK CPU pegged near 150% at idle.** Root cause was CSS compositor thrash, not JS frame work. The fullscreen scanline overlay's `mix-blend-mode`, per-core/per-stat `filter: blur` fills, `transition: width` on bars updating every tick, the `.flash` keyframe animation, and `ease-*` curves on infinite blinks (now `step-end`) all hammered the WebKitWebProcess compositor. Each was removed or swapped; idle GUI CPU drops from ~150% to single digits on Ubuntu 24.04 + WebKitGTK 4.1.
- **Linux tray menu rendered with blank labels under the GNOME AppIndicator extension.** A race in libayatana-appindicator's dbusmenu attach meant menu items appeared but their text never populated. The `set_menu` call is now deferred by ~1.5s via `run_on_main_thread`, dodging the race. Stock `tray-icon` 0.21.3 is back from crates.io; the local patch is gone.

### Changed
- **Snapshot delivery moved from polling to push.** The frontend no longer calls `get_snapshot` on `setInterval`; the backend now emits `dofek://snapshot` Tauri events each tick and the frontend listens. Eliminates the per-tick IPC round-trip and the full-snapshot JSON serialize/parse on the JS side.
- Bezier smoothing removed from the four small sparklines (still applied to the main chart) — cuts paint cost without visibly degrading them.
- **Release workflow restored to `draft: true`**, reverting the v1.3.1 change. Maintainer review of built assets before publication is the desired workflow; retags flipping a published release back to draft is the lesser concern.
- Help-overlay settings list collapsed into one section (was visually fragmented).

### Notes
- 1.3.2 was skipped — the version bump landed in the same commit as the WebKitGTK fix.
- macOS GPU/VRAM and CPU temp/power remain N/A; tracked for a future release.

## [1.3.1] - 2026-04-27

Patch release — UX polish around the v1.3 tray companion and a long-overdue terminal-capability check on the TUI.

### Added
- **TUI startup truecolor check.** dofek-tui now detects whether the host terminal advertises 24-bit RGB (`COLORTERM=truecolor|24bit`) and prints a clear warning, with a 3-second pause, before entering the alternate screen if it doesn't. The warning lists known-good alternatives (iTerm2, WezTerm, Ghostty, Alacritty, Kitty). Apple's Terminal.app — which has misparsed the truecolor SGR sequences for ~a decade — was the original prompt; users no longer get a wall of magenta-on-red panels with no explanation. `NO_COLOR` is also respected.

### Fixed
- **Tray "Settings" menu item now opens the help overlay.** The Rust side emitted `dofek://open-settings` when Settings was clicked, but the frontend never listened — the menu item silently did nothing. Wired up a Tauri event listener on the frontend that opens the help overlay (where the tray toggles live) and hydrates the toggle state from `get_settings`.
- **Release pipeline no longer un-publishes its own releases.** `release.yml` had `draft: true` hardcoded on the `softprops/action-gh-release@v3` step, which on every retag re-set an already-published release back to draft — silently 404-ing all asset downloads. Switched to `draft: false`. Trigger is already restricted to semver tag pushes; if a future release needs review, it can be marked draft via `gh release edit` after the fact.

### Notes
- Code-side palette is unchanged. A 256-color fallback (so the TUI renders correctly even without truecolor) remains tracked for v1.4.

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
