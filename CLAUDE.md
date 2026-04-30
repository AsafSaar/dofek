# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**dofek** (דּוֹפֶק — Hebrew for "pulse") is a dual-interface, AI-aware system monitor for Windows, Linux, and macOS (Apple Silicon), built with Rust. The TUI uses Ratatui + crossterm, the GUI uses Tauri 2 (WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS). Both share a common core library for data collection. It uses the `sysinfo` crate for CPU/memory/process/network/hostname data, NVML for NVIDIA GPU metrics and per-process VRAM, and a plugin system for extensibility via JSON-over-stdio.

Targets: Windows 11 (Windows 10 build 19041+), Linux x86_64 (Ubuntu 24.04, Fedora 40, Arch), macOS 12+ on Apple Silicon (`aarch64-apple-darwin` only — Intel Macs are not supported). Single binary per interface, no runtime dependencies.

## Build & Run

Cargo aliases are defined in `.cargo/config.toml` — run all commands from the repo root.

```bash
# Dev (debug, fast compile)
cargo tui                          # Run TUI
cargo gui                          # Run GUI (hot-reload)

# Release builds (LTO + strip)
cargo build-tui                    # → target/release/dofek-tui[.exe]
cargo build-gui                    # → target/release/dofek-gui[.exe] + native bundles

# Native installer / packages (bundles both TUI + GUI)
.\build-all.ps1                    # Windows → target/release/bundle/msi/dofek_*.msi
./build-all.sh                     # Linux   → target/release/bundle/{deb,rpm,appimage}/dofek_*
```

**Prerequisites:** Rust toolchain (stable, edition 2024), Tauri CLI (`cargo install tauri-cli --version "^2"`) for GUI builds, plus per-OS:
- **Windows:** Visual Studio Build Tools with C++ workload.
- **Linux (apt):** `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libgtk-3-dev` — and `rpm` if you want `.rpm` bundles.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`). Apple Silicon only — Intel Macs are not supported.

**Optional for enhanced functionality:**
- NVIDIA GPU + drivers for GPU metrics and per-process VRAM (NVML — `nvml.dll` on Windows, `libnvidia-ml.so` on Linux). Gracefully degrades without it.
- **Windows only:** LibreHardwareMonitor with web server on port 8085 — fallback for CPU temp/power and non-NVIDIA GPU data. On Linux, dofek reads CPU temps directly from `/sys/class/hwmon` via `sysinfo::Components`, so LHM is not needed.

## Architecture

### Dual-Interface Model
```
    dofek (workspace)
    ├── dofek (lib + TUI binary)
    │     ├── sysinfo crate ──── CPU, memory, processes (with CPU%)
    │     ├── NVML ──────────── GPU metrics + per-process VRAM (NVIDIA, multi-GPU)
    │     ├── LHM HTTP ─────── GPU fallback (optional, non-NVIDIA)
    │     ├── Windows API ───── network stats (GetIfTable2), local time
    │     ├── Plugin system ─── JSON-over-stdio child process plugins
    │     └── Ratatui TUI ───── rendering (trading-terminal layout)
    │
    ├── dofek-gui (Tauri 2 desktop app)
    │     ├── Reuses dofek core lib for data collection + plugins
    │     ├── Tauri IPC ────── get_snapshot / get_gpu_info / settings commands
    │     └── Vanilla HTML/CSS/JS frontend with Canvas charts
    │
    └── plugins/
          ├── dofek-ollama ──── Ollama model status + inference tracking
          ├── dofek-docker ──── Docker container monitoring
          └── dofek-net-ping ── TCP-connect latency sampler
```

### Threading Model (sync, no tokio)
- **Main thread**: Render loop + event handling. Receives data via `mpsc::channel`.
- **Data collector thread** (`data::spawn_collector`): Refreshes sysinfo, queries NVML, enumerates network, polls plugins. Sends `DataSnapshot` over channel. The `sysinfo::System` instance lives here (persists across polls for CPU% delta computation).
- **Event reader thread** (`event::spawn_event_reader`): Reads crossterm keyboard events, sends `AppEvent` over channel.

### Module Structure

- `src/main.rs` — Entry point: terminal init, thread spawning, main event/render loop
- `src/lib.rs` — Shared library (used by both TUI and GUI)
- `src/app.rs` — App state: `DataSnapshot`, `HistoryBuffers`, `ChartTab`, `CategoryFilter`, `GpuTab`
- `src/config.rs` — CLI (clap) + TOML config loading with `[categories]` and `[[plugins]]` sections
- `src/settings.rs` — User settings (persisted to `%APPDATA%/dofek/`)
- `src/event.rs` — Crossterm event reader thread, `AppEvent` enum
- `src/data/` — Data collection layer:
  - `mod.rs` — `DataSnapshot` struct (with `gpus: Vec<GpuSensors>`), collector thread
  - `sysinfo_source.rs` — sysinfo-backed CPU, memory, and process extraction
  - `gpu.rs` — NVML wrapper: multi-GPU device metrics + per-process VRAM
  - `lhm.rs` — LHM HTTP client (optional GPU fallback, multi-GPU aware)
  - `process.rs` — `ProcessInfo`, `AiState`, `ProcessCategory` definitions
  - `network.rs` — Per-interface rx/tx bytes with delta-based rate computation. Windows uses `GetIfTable2`; Linux and macOS share the `sysinfo::Networks` path. The macOS branch additionally filters Apple-internal pseudo-interfaces (`awdl0`, `llw0`, `gif0`, `stf0`, `anpi0`/`1`, `ap1`). All three platforms share the `NetworkTracker` state struct.
  - `ai_detect.rs` — AI workload + category classification (AI/DEV/WATCH)
- `src/plugin/` — Plugin system:
  - `mod.rs` — `PluginManager`: spawn, poll, restart, shutdown
  - `protocol.rs` — Serde structs for JSON request/response protocol
  - `process.rs` — Child process wrapper: stdio pipes, timeout, Job Object
- `src/ui/` — Rendering layer (trading-terminal layout):
  - `mod.rs` — Master layout: ticker + chart/watchlist split + bottom strip + status bar
  - `theme.rs` — Trading-terminal color palette (sky blue CPU, violet GPU, emerald MEM, etc.)
  - `ticker.rs` — Top ticker bar with metric pills, AI badge, hostname, clock (uses `chrono::Local` cross-platform)
  - `chart.rs` — Main chart panel with tab switching (CPU/GPU/MEM/NET)
  - `candlestick.rs` — Custom candlestick widget (Buffer manipulation, half-blocks)
  - `area_chart.rs` — Custom area chart widget (filled, multi-series, thresholds)
  - `horizon_chart.rs` — Custom horizon chart widget (3-band color-intensity layering)
  - `watchlist.rs` — Process watchlist with category tabs, sort buttons, plugin dock
  - `bottom_strip.rs` — Compact 4-panel row: CPU core grid, GPU stats, MEM bars, NET rates
  - `status.rs` — Bottom status bar with keybindings
  - `sparkline_buf.rs` — Ring buffers: `SparklineBuf` (u64) + `CandleBuf` (OHLC-style candles)
  - `cpu.rs`, `gpu.rs`, `memory.rs`, `network_disk.rs` — Panel renderers (full-screen mode)
  - `process_table.rs` — Full-screen process table (via `p` key)
  - `help.rs` — Help overlay popup
  - `about.rs` — About overlay
  - `header.rs`, `footer.rs` — Header/footer renderers

### GUI Structure

- `gui/src/lib.rs` — Tauri backend: `AppState`, IPC commands, data collector thread
- `gui/frontend/index.html` — Single-file frontend: HTML + CSS + Canvas charts + JS
- `gui/tauri.conf.json` — Tauri app config (bundle, window, CSP, externalBin)
- `gui/icons/icon.ico` — App icon (pulse heartbeat, multi-size)
- `gui/icons/icon.png` — App icon PNG (256x256)

### Website

- `website/index.html` — Landing page (dofek.dev)
- `website/plugins/index.html` — Plugin development docs with interactive playground
- `website/plugins/style.css` — Plugin page styles
- `website/plugins/playground.js` — Live JSON editor + plugin scaffolder
- `website/favicon.svg` — Pulse heartbeat favicon
- `website/robots.txt`, `website/sitemap.xml` — SEO

### Build Scripts

- `.cargo/config.toml` — Cargo aliases (`cargo tui`, `cargo gui`, `cargo build-tui`, `cargo build-gui`)
- `build-all.ps1` — PowerShell: builds TUI + GUI, packages single MSI installer
- `build-all.sh` — Bash equivalent (may not work in Git Bash due to PATH issues; use .ps1)

### Key Data Flow
`sysinfo refresh → extract_cpu/extract_memory/enumerate_processes → DataSnapshot → App.update_data() → HistoryBuffers → ui::render()`

GPU data flow: `NVML query → GpuDeviceInfo + per_process_vram → GpuSensors` (or LHM fallback if NVML unavailable)

Plugin data flow: `PluginManager.poll() → JSON stdin/stdout → panels + process_annotations + metrics → DataSnapshot`

### LHM JSON Structure (optional fallback)
The `/data.json` endpoint returns a recursive tree of `LhmNode` objects with `Text`, `Value`, `Children` fields. Values are strings like `"64.3 %"` or `"1200 MHz"` that need `parse_lhm_value()` to extract the numeric part.

## Config (dofek.toml)

See `dofek.toml.example` for all options. Key settings:
- `general.refresh_ms` (default 500) — poll interval
- `ai.known_ai_processes` — list of process names treated as AI workloads
- `ai.vram_threshold_gb` (default 1.0) — VRAM usage above this flags a process as AI
- `categories.dev_processes` — process names classified as DEV
- `categories.watch_processes` — process names pinned as WATCH
- `lhm.url` (default `http://localhost:8085`) — LHM web server address (only used as GPU fallback)
- `[[plugins]]` — plugin definitions (name, command, args, enabled, timeout_ms)

## Current Status (v1.5)

Trading-terminal layout with dual interface (TUI + Tauri GUI), candlestick CPU chart, area/horizon charts for GPU/MEM/NET/DISK, multi-GPU support, process categories (AI/DEV/WATCH), top ticker bar, compact bottom strip, plugin system with JSON-over-stdio protocol and a managed plugin store (install via GUI file picker or `dofek-tui plugins add`, hot-reloaded on `plugins.toml` change), system-tray companion (live CPU sparkline icon, close-to-tray, macOS menu-bar text), Linux CPU power via RAPL, cross-platform disk I/O metrics. Custom chart widgets use Buffer manipulation with half-block characters for 2x vertical resolution.

Keybindings (TUI): q/tab/p/c/g/m/n/d/h/1-4/esc/?/+/-/s/a/[/].

### Known Limitations
- AMD GPU VRAM not supported (NVML is NVIDIA-only; on Windows, the LHM fallback provides basic GPU data)
- **Windows:** CPU temperature/power not available without LHM (sysinfo doesn't provide these on Windows without elevation)
- **Linux:** CPU temperature works via sysinfo::Components (reads /sys/class/hwmon). CPU power available on Intel via RAPL (`/sys/class/powercap/intel-rapl:0/energy_uj`); falls back silently on AMD or hardened distros where the sysfs read is denied.
- **macOS:** GPU/VRAM and CPU temperature/power are still not implemented in v1.5 — those panels show N/A. NVML is NVIDIA-only, and Apple Silicon SMC sensor coverage in `sysinfo` is not yet sufficient. CPU, memory, network, disk, processes, and process kill all work.
- **Linux GNOME tray:** GNOME removed legacy tray support. The dofek tray icon needs the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/) installed; without it the tray won't appear (other features unaffected).
- macOS support is Apple Silicon only — Intel Macs and universal binaries are not part of this release. Linux ARM64 is also tracked for the future.
