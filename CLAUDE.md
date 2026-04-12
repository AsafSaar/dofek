# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**dofek** (דּוֹפֶק — Hebrew for "pulse") is a terminal-native, AI-aware system monitor for Windows, built with Rust + Ratatui. It reads hardware sensors from LibreHardwareMonitor's HTTP API, enumerates processes via Windows API, queries per-process VRAM via NVML, and renders a multi-panel TUI dashboard.

Target: Windows 11 (Windows 10 build 19041+). Single binary, no runtime dependencies.

## Build & Run

```bash
# Requires: Rust toolchain (rustup), Visual Studio Build Tools with C++ ARM64 tools
# The target is aarch64-pc-windows-msvc (ARM64 Windows)

cargo build              # Debug build
cargo build --release    # Release build (LTO + strip)
cargo run                # Run (LHM must be running on localhost:8085 for full data)
```

**Prerequisites for full functionality:**
- LibreHardwareMonitor running with web server enabled on port 8085
- NVIDIA GPU + drivers for per-process VRAM (NVML). Gracefully degrades without it.

**Note:** `ureq` is configured with `default-features = false` (no TLS) since LHM is localhost-only HTTP.

## Architecture

### Two-Process Model
```
LibreHardwareMonitor (elevated, HTTP on :8085)
         │  GET /data.json every 500ms
    dofek (unprivileged Rust binary)
         ├── Windows API for processes
         ├── NVML for per-process VRAM
         └── Ratatui TUI rendering
```

### Threading Model (sync, no tokio)
- **Main thread**: Render loop + event handling. Receives data via `mpsc::channel`.
- **Data collector thread** (`data::spawn_collector`): Polls LHM, enumerates processes, queries NVML. Sends `DataSnapshot` over channel.
- **Event reader thread** (`event::spawn_event_reader`): Reads crossterm keyboard events, sends `AppEvent` over channel.

### Module Structure

- `src/main.rs` — Entry point: terminal init, thread spawning, main event/render loop
- `src/app.rs` — App state model: holds `DataSnapshot`, `HistoryBuffers`, focus/sort state
- `src/config.rs` — CLI (clap) + TOML config loading. Lookup order: `--config` flag → `./dofek.toml` → `%APPDATA%/dofek/dofek.toml`
- `src/event.rs` — Crossterm event reader thread, `AppEvent` enum
- `src/data/` — Data collection layer:
  - `mod.rs` — `DataSnapshot` struct, collector thread orchestration
  - `lhm.rs` — LHM HTTP client, `LhmNode` tree deserialization, sensor extraction (CPU/Memory/GPU)
  - `process.rs` — Windows `EnumProcesses` + `GetProcessMemoryInfo` + `GetModuleBaseNameW`
  - `gpu.rs` — NVML wrapper: per-process VRAM via `running_compute_processes()`/`running_graphics_processes()`
  - `network.rs` — `GetIfTable2` for per-interface rx/tx bytes, delta computation
  - `ai_detect.rs` — AI workload classification (name match + VRAM threshold + GPU util)
- `src/ui/` — Rendering layer (all render functions take `&App` and write to `Frame`):
  - `mod.rs` — Master layout, dashboard view splits, panel dispatch by focus state
  - `theme.rs` — Color palette constants (hex values from spec)
  - `header.rs`, `footer.rs`, `cpu.rs`, `memory.rs`, `gpu.rs`, `network_disk.rs`, `process_table.rs`, `help.rs`
  - `sparkline_buf.rs` — Ring buffer (`VecDeque<u64>`) for sparkline history

### Key Data Flow
`LHM JSON → LhmNode tree → extract_cpu/extract_memory/extract_gpu → DataSnapshot → App.update_data() → HistoryBuffers → ui::render()`

### LHM JSON Structure
The `/data.json` endpoint returns a recursive tree of `LhmNode` objects with `Text`, `Value`, `Children` fields. Values are strings like `"64.3 %"` or `"1200 MHz"` that need `parse_lhm_value()` to extract the numeric part.

## Config (dofek.toml)

See `dofek.toml.example` for all options. Key settings:
- `general.refresh_ms` (default 500) — poll interval
- `ai.known_ai_processes` — list of process names treated as AI workloads
- `ai.vram_threshold_gb` (default 1.0) — VRAM usage above this flags a process as AI
- `lhm.url` (default `http://localhost:8085`) — LHM web server address

## Current Status (v0.1 POC)

All panels implemented: CPU, Memory, GPU, Network+Disk, Process Table with VRAM column and AI badges. Keybindings: q/tab/p/g/c/m/esc/?/+/-/s.

### Known Limitations
- CPU% per-process is not computed (placeholder 0.0) — needs kernel/user time delta tracking
- AMD GPU VRAM not supported (NVML is NVIDIA-only)
- No disk I/O stats yet in the network_disk panel
- Windows-only (intentional for v0.1)
