# dofek-ollama

Dofek plugin that surfaces [Ollama](https://ollama.com) model status in the dashboard.

## What it shows

- **Plugin dock panel (`OLLAMA`)** — currently loaded model(s) with size, plus the count of available models pulled locally.
- **Ticker pill (`Models`)** — live count of running models (`ollama.running` metric).
- **Process annotation** — tags `ollama` processes with `category=ai`, the loaded model name, and an `ai_state` of `idle` while a model is resident.

When the Ollama daemon isn't reachable the panel shows `offline` in dim style and the rest of Dofek keeps running normally.

## Prerequisites

- Ollama running on `http://localhost:11434` (the default — start with `ollama serve`).
- Rust toolchain (if building from source).

## Build & install

From the repo root:

```bash
cargo build --release -p dofek-ollama
```

Then register it with Dofek, which copies it into the managed plugin
directory (`<config_dir>/dofek/plugins/`):

```bash
dofek-tui plugins add target/release/dofek-ollama
```

The GUI equivalent is **Settings (`?`) → Plugins → + Add plugin**.

`PATH` is never searched — `command` must be either a bare file name in the
managed directory or an absolute path. See
[SECURITY.md](../../SECURITY.md#plugin-security) for why.

> **You may not need to build this at all.** `dofek-ollama` ships inside the
> MSI, `.dmg`/`.app` and AppImage, so on those installs a bare
> `command = "dofek-ollama"` resolves to the bundled copy with nothing to
> install.

## Configuration

Add this block to your `dofek.toml`:

```toml
[[plugins]]
name = "ollama"
command = "dofek-ollama"
args = ["--host", "http://localhost:11434"]
enabled = true
timeout_ms = 2000
```

### CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `http://localhost:11434` | Base URL of the Ollama HTTP API. `--port` is accepted as an alias. |

## Troubleshooting

- **Panel stuck on `offline`** — confirm `curl http://localhost:11434/api/tags` returns JSON. If Ollama is bound to a non-default port or remote host, pass it via `--host`.
- **No process annotation appears** — Dofek only annotates processes whose name contains `ollama`. On macOS the process is `ollama`, on Linux it may be `ollama-runner` or `ollama_llama_server` — both match.
- **Plugin dock indicator is yellow** — five consecutive poll errors in a row. Check Dofek's stderr log; the plugin reports HTTP failures there.

## See also

- Plugin protocol reference: [`../README.md`](../README.md)
- Live JSON playground: <https://dofek.dev/plugins/>
- Source: [`src/main.rs`](src/main.rs)
