# dofek

**Terminal-native, AI-aware system monitor for Windows.**

> *dofek* (Hebrew: דּוֹפֶק) means "pulse" or "heartbeat"

Most system monitors were designed before LLMs ran locally. They treat GPU as an afterthought and VRAM as a footnote. dofek is built for the developer who has `ollama` running in the background, is watching a model load into VRAM, and needs to know at a glance whether their system can handle the next task.

```
┌─────────────────────────────────────────────────────────────────────┐
│ dofek v0.1                 ● ollama inferring            HOSTNAME  │
├───────────────────────────┬─────────────────────────────────────────┤
│ CPU Intel Core i7-13700K  │ MEM 24.3 / 64.0 GB                    │
│ C0  ████████░░░░░  62.1%  │ Used ██████████░░░░  68.2%             │
│ C1  ███░░░░░░░░░░  23.4%  │ Swap █░░░░░░░░░░░░░   8.1%            │
│ C2  █████████████  98.7%  │                                        │
│ ▁▂▃▅▇█▇▅▃▂▁▂▃▅▇█ total   │ ▁▂▃▄▄▅▅▅▅▅▅▅▆▆▆▆▆▆ used              │
├───────────────────────────┼─────────────────────────────────────────┤
│ GPU RTX 4090 · 24576 MB   │ NET + DISK                             │
│ Util ███████░░░░░  55.2%  │ Realtek Gaming 2.5GbE                  │
│ VRAM █████████░░░ 18.2 GB │  ↓ 12.4 MB/s   ↑ 1.2 MB/s             │
│ Temp ████░░░░░░░░  67°C   │                                        │
│ ▁▃▅▇█▇▅▃▅▇█▇▅▃▅▇ util    │ ▁▁▂▃▅▇█▇▅▃▂▁▁▂▃▅▇█ rx                │
├───────────────────────────┴─────────────────────────────────────────┤
│ PROCESSES                                              sort: MEM   │
│ NAME                    PID   CPU%     MEM      VRAM   AI          │
│ ollama_llama_server    1234   12.3   2.1 GB  18.0 GB  ● infer     │
│ chrome.exe             5678    8.1   1.8 GB       —                │
│ python.exe             9012    4.2   1.2 GB   2.1 GB  ○ idle      │
├─────────────────────────────────────────────────────────────────────┤
│ q quit  tab sort  p proc  g gpu  c cpu  m mem  ? help       500ms │
└─────────────────────────────────────────────────────────────────────┘
```

## Features

- **AI-first monitoring** — VRAM per-process, AI workload detection, inference/loading/idle badges
- **Full hardware dashboard** — CPU per-core, memory, GPU (util/VRAM/temp/power), network
- **Process table with VRAM column** — the column Windows Task Manager doesn't have
- **Sparkline history** — 60-sample rolling graphs for CPU, memory, GPU, and network
- **Single binary** — one `.exe`, no runtime dependencies
- **Configurable** — TOML config for refresh rate, AI process names, display options

## Requirements

- **Windows 10** (build 19041+) or **Windows 11**
- **[LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases)** — for CPU/GPU/memory sensor data
  - Download the latest release ZIP, extract, and run as administrator
  - Enable the web server: **Options > Remote Web Server > Run** (default port 8085)
- **NVIDIA GPU + drivers** *(optional)* — for per-process VRAM via NVML. Gracefully degrades without it.

## Install

### From source

```bash
git clone https://github.com/AsafSaar/dofek.git
cd dofek
cargo build --release
# Binary at target/release/dofek.exe
```

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with C++ workload

## Usage

```bash
# Start LibreHardwareMonitor first (as admin, with web server on port 8085)
# Then run dofek:
dofek

# With a custom config:
dofek --config path/to/dofek.toml
```

## Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `tab` | Cycle sort column in process table |
| `p` | Focus process table (full screen) |
| `g` | Focus GPU panel (full screen) |
| `c` | Focus CPU panel (full screen) |
| `m` | Focus memory panel (full screen) |
| `esc` | Return to dashboard |
| `?` | Toggle help overlay |
| `+` / `-` | Increase / decrease refresh rate |
| `s` | Save snapshot to file |

## Configuration

Config is loaded from (in order): `--config` flag, `./dofek.toml`, `%APPDATA%\dofek\dofek.toml`.

```toml
[general]
refresh_ms = 500          # Poll interval in milliseconds
history_len = 60          # Number of sparkline samples to keep

[display]
show_temps = true         # Show temperature bars
show_power = true         # Show power draw bars
process_count = 10        # Max processes to display

[ai]
vram_threshold_gb = 1.0   # VRAM above this flags a process as AI
known_ai_processes = ["ollama", "ollama_llama_server", "python", "lm_studio"]

[lhm]
url = "http://localhost:8085"  # LibreHardwareMonitor web server
```

## AI Workload Detection

A process is classified as an AI workload if:

1. Its name matches `known_ai_processes` (case-insensitive), **or**
2. Its VRAM usage exceeds `vram_threshold_gb`, **or**
3. Its name ends with `_server` and it uses any VRAM

| Badge | Condition |
|-------|-----------|
| `● inferring` | VRAM > threshold **and** GPU utilization > 20% |
| `● loading` | VRAM increasing rapidly (>200 MB in last poll) |
| `○ idle` | Known AI process but VRAM < 500 MB |

## Architecture

```
LibreHardwareMonitor (elevated, HTTP on :8085)
         │  GET /data.json
    dofek (unprivileged)
         ├── Data collector thread → polls LHM, Windows API, NVML
         ├── Event reader thread   → crossterm keyboard events
         └── Main thread           → render loop (Ratatui)
                    ↑ mpsc::channel
```

Sync threaded model — no async runtime. Three threads communicate via `mpsc::channel`.

## Roadmap

- **v0.1** (current) — Full dashboard with real sensor data, AI workload detection
- **v0.2** — Plugin system (stdout JSON protocol), `dofek-ollama` and `dofek-docker` plugins
- **v0.3** — User themes and configurable panel layout
- **v1.0** — GUI tray companion with live sparkline in taskbar

## License

MIT
