# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**dofek** (דּוֹפֶק — Hebrew for "pulse") is a terminal-native, AI-aware system monitor for Windows, built with Rust + Ratatui. It uses the `sysinfo` crate for CPU/memory/process data, NVML for NVIDIA GPU metrics and per-process VRAM, and renders a multi-panel TUI dashboard. LibreHardwareMonitor is an optional fallback for GPU data on non-NVIDIA systems.

Target: Windows 11 (Windows 10 build 19041+). Single binary, no runtime dependencies.

## Build & Run

```bash
# Requires: Rust toolchain (rustup), Visual Studio Build Tools with C++ ARM64 tools
# The target is aarch64-pc-windows-msvc (ARM64 Windows)

cargo build              # Debug build
cargo build --release    # Release build (LTO + strip)
cargo run                # Run (works out of the box — no external dependencies required)
```

**Optional for enhanced functionality:**
- NVIDIA GPU + drivers for GPU metrics and per-process VRAM (NVML). Gracefully degrades without it.
- LibreHardwareMonitor with web server on port 8085 — optional fallback for GPU data on non-NVIDIA systems.

## Architecture

### Single-Process Model
```
    dofek (unprivileged Rust binary)
         ├── sysinfo crate for CPU, memory, processes (with CPU%)
         ├── NVML for GPU metrics + per-process VRAM (NVIDIA)
         ├── LHM HTTP fallback for GPU (optional, non-NVIDIA)
         ├── Windows API for network stats
         └── Ratatui TUI rendering
```

### Threading Model (sync, no tokio)
- **Main thread**: Render loop + event handling. Receives data via `mpsc::channel`.
- **Data collector thread** (`data::spawn_collector`): Refreshes sysinfo, queries NVML, enumerates network. Sends `DataSnapshot` over channel. The `sysinfo::System` instance lives here (persists across polls for CPU% delta computation).
- **Event reader thread** (`event::spawn_event_reader`): Reads crossterm keyboard events, sends `AppEvent` over channel.

### Module Structure

- `src/main.rs` — Entry point: terminal init, thread spawning, main event/render loop
- `src/app.rs` — App state: `DataSnapshot`, `HistoryBuffers`, `ChartTab`, `CategoryFilter`, `GpuTab`
- `src/config.rs` — CLI (clap) + TOML config loading with `[categories]` section
- `src/event.rs` — Crossterm event reader thread, `AppEvent` enum
- `src/data/` — Data collection layer:
  - `mod.rs` — `DataSnapshot` struct (with `gpus: Vec<GpuSensors>`), collector thread
  - `sysinfo_source.rs` — sysinfo-backed CPU, memory, and process extraction
  - `gpu.rs` — NVML wrapper: multi-GPU device metrics + per-process VRAM
  - `lhm.rs` — LHM HTTP client (optional GPU fallback, multi-GPU aware)
  - `process.rs` — `ProcessInfo`, `AiState`, `ProcessCategory` definitions
  - `network.rs` — `GetIfTable2` for per-interface rx/tx bytes, delta computation
  - `ai_detect.rs` — AI workload + category classification (AI/DEV/WATCH)
- `src/ui/` — Rendering layer (trading-terminal layout):
  - `mod.rs` — Master layout: ticker + chart/watchlist split + bottom strip + status bar
  - `theme.rs` — Trading-terminal color palette (sky blue CPU, violet GPU, emerald MEM, etc.)
  - `ticker.rs` — Top ticker bar with metric pills, AI badge, hostname, clock
  - `chart.rs` — Main chart panel with tab switching (CPU/GPU/MEM/NET)
  - `candlestick.rs` — Custom candlestick widget (Buffer manipulation, half-blocks)
  - `area_chart.rs` — Custom area chart widget (filled, multi-series, thresholds)
  - `watchlist.rs` — Process watchlist with category tabs, sort buttons, plugin dock
  - `bottom_strip.rs` — Compact 4-panel row: CPU core grid, GPU stats, MEM bars, NET rates
  - `status.rs` — Bottom status bar with keybindings
  - `sparkline_buf.rs` — Ring buffers: `SparklineBuf` (u64) + `CandleBuf` (OHLC-style candles)
  - `cpu.rs`, `gpu.rs`, `memory.rs`, `network_disk.rs` — Panel renderers (full-screen mode)
  - `process_table.rs` — Full-screen process table (via `p` key)
  - `help.rs` — Help overlay popup

### Key Data Flow
`sysinfo refresh → extract_cpu/extract_memory/enumerate_processes → DataSnapshot → App.update_data() → HistoryBuffers → ui::render()`

GPU data flow: `NVML query → GpuDeviceInfo + per_process_vram → GpuSensors` (or LHM fallback if NVML unavailable)

### LHM JSON Structure (optional fallback)
The `/data.json` endpoint returns a recursive tree of `LhmNode` objects with `Text`, `Value`, `Children` fields. Values are strings like `"64.3 %"` or `"1200 MHz"` that need `parse_lhm_value()` to extract the numeric part.

## Config (dofek.toml)

See `dofek.toml.example` for all options. Key settings:
- `general.refresh_ms` (default 500) — poll interval
- `ai.known_ai_processes` — list of process names treated as AI workloads
- `ai.vram_threshold_gb` (default 1.0) — VRAM usage above this flags a process as AI
- `lhm.url` (default `http://localhost:8085`) — LHM web server address (only used as GPU fallback)

## Current Status (v0.2)

Trading-terminal redesign complete: two-zone layout (main chart + watchlist), candlestick CPU chart, area charts for GPU/MEM/NET, multi-GPU support, process categories (AI/DEV/WATCH), top ticker bar, compact bottom strip. Custom chart widgets use Buffer manipulation with half-block characters for 2x vertical resolution.

Keybindings: q/tab/p/c/g/m/n/1-4/esc/?/+/-/s.

### Known Limitations
- AMD GPU VRAM not supported (NVML is NVIDIA-only; LHM fallback provides basic GPU data)
- No disk I/O stats yet
- CPU temperature/power not available without LHM (sysinfo doesn't provide these on Windows without elevation)
- Windows-only (intentional)
- Plugin dock is UI placeholder only (no plugin system yet)
