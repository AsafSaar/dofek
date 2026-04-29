# Dofek Plugin Development Guide

> ⚠️ **Plugin API: experimental, subject to change until further notice.**
> The JSON protocol is versioned (`schema_version: 1`), but expect breaking changes as the API matures. Pin your plugin to a specific Dofek version if stability matters. Once Dofek's plugin contract stabilizes, it will follow semver.

Build external plugins that inject data into the Dofek dashboard. Plugins are standalone executables that communicate with Dofek via JSON-over-stdio.

## How It Works

```
Dofek (parent)                    plugin (child process)
  │                                  │
  │── spawns process ───────────────>│
  │                                  │
  │   [every refresh cycle]          │
  │── {"type":"poll",...}\n ────────>│
  │<── {"type":"poll_response"}\n ──│
  │                                  │
  │   [on exit]                      │
  │── {"type":"shutdown"}\n ───────>│
  │── kills after 2s ──────────────>│
```

- **Pull model**: Dofek writes a request to the plugin's **stdin**, reads a response from **stdout**
- **Newline-delimited JSON**: one JSON object per line, terminated by `\n`
- **stderr**: captured by Dofek for logging — use it for debug output
- **Timeout**: if a plugin doesn't respond within `timeout_ms` (default 2000ms), the poll is skipped
- **Crash recovery**: if the process dies, Dofek restarts it with exponential backoff (1s, 2s, 4s, 8s, 16s, 30s cap)

## Configuration

Add a `[[plugins]]` entry to `dofek.toml`:

```toml
[[plugins]]
name = "my-plugin"            # Display name (used if no manifest provided)
command = "dofek-my-plugin"   # Binary name (resolved via PATH) or absolute path
args = ["--flag", "value"]    # Optional arguments
enabled = true                # Default: true
timeout_ms = 2000             # Per-poll timeout in milliseconds (default: 2000)
```

Multiple `[[plugins]]` entries are supported. Order determines dock layout order.

## Protocol Reference

### Poll Request (Dofek → plugin)

Sent to **stdin** on every refresh cycle:

```json
{
  "type": "poll",
  "timestamp_ms": 1713020400000,
  "processes": [
    { "pid": 1234, "name": "ollama_llama_server.exe", "vram_bytes": 4294967296 },
    { "pid": 5678, "name": "python.exe", "vram_bytes": null }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | `"poll"` | Always `"poll"` |
| `timestamp_ms` | `u64` | Unix timestamp in milliseconds |
| `processes` | `array` | Current system processes — ignore if your plugin doesn't need them |

Each process object:

| Field | Type | Description |
|-------|------|-------------|
| `pid` | `u32` | Process ID |
| `name` | `string` | Process name (e.g., `"ollama.exe"`) |
| `vram_bytes` | `u64 \| null` | GPU VRAM usage in bytes, or null if unknown |

### Poll Response (plugin → Dofek)

Write to **stdout** as a single JSON line:

```json
{
  "status": "ok",
  "panels": [ ... ],
  "process_annotations": [ ... ],
  "metrics": [ ... ]
}
```

All three arrays are **optional** — include only what your plugin provides.

#### `manifest` (first response only)

Include a `manifest` field in your first response so Dofek can identify the plugin:

```json
{
  "status": "ok",
  "manifest": {
    "name": "dofek-my-plugin",
    "version": "0.1.0",
    "description": "What this plugin does",
    "author": "Your Name"
  },
  "panels": []
}
```

The `manifest.name` overrides the config `name` in the UI. Only read once — omit it on subsequent responses.

#### `panels` — Plugin Dock UI

Key-value data displayed in the plugin dock at the bottom of the watchlist:

```json
"panels": [
  {
    "id": "my-panel",
    "label": "MY PLUGIN",
    "content": [
      { "key": "Status", "value": "running", "style": "accent" },
      { "key": "Items", "value": "12 active", "style": "normal" }
    ]
  }
]
```

The first panel's content is shown inline next to the plugin name in the dock. The `label` is displayed as the panel header.

**Style values:**

| Style | Color | Use for |
|-------|-------|---------|
| `"normal"` | Light gray | Default text |
| `"accent"` | Sky blue | Highlighted values |
| `"dim"` | Dark gray | Secondary information |
| `"warn"` | Amber | Warnings |
| `"error"` | Red | Errors |

#### `process_annotations` — Enrich Process Rows

Override or add labels to processes in the watchlist:

```json
"process_annotations": [
  {
    "pid": 1234,
    "label": "llama3:8b",
    "category": "ai",
    "ai_state": "inferring"
  }
]
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pid` | `u32` | yes | Must match a running process PID |
| `label` | `string` | no | Displayed as `plugin_label` on the process row |
| `category` | `string` | no | Override category: `"ai"`, `"dev"`, or `"watch"` |
| `ai_state` | `string` | no | Override AI state: `"idle"`, `"loading"`, or `"inferring"` |

All fields except `pid` are optional — include only what you want to override.

#### `metrics` — Ticker Bar Pills

Named numeric values displayed as pills in the top ticker bar:

```json
"metrics": [
  {
    "id": "my_plugin.active_items",
    "label": "Items",
    "value": 12.0,
    "unit": ""
  }
]
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Unique metric identifier (namespaced, e.g., `"ollama.running"`) |
| `label` | `string` | Short label shown in the ticker |
| `value` | `f64` | Numeric value |
| `unit` | `string` | Unit suffix (e.g., `"req/m"`, `"MB"`) — empty string for no unit |

### Shutdown Request

Sent when Dofek is exiting. The plugin should clean up and exit:

```json
{ "type": "shutdown" }
```

Dofek waits 2 seconds after sending this, then kills the process if still running.

## Plugin States

Dofek tracks each plugin's health:

| State | Dock indicator | Condition |
|-------|---------------|-----------|
| Starting | `○` gray | Just spawned, no successful poll yet |
| Healthy | `●` green | Last poll succeeded |
| Unhealthy | `●` yellow | 5+ consecutive poll errors/timeouts |
| Crashed | `●` red | Process exited unexpectedly |

After a crash, Dofek respawns the plugin with exponential backoff: 1s → 2s → 4s → 8s → 16s → 30s (capped).

## Minimal Plugin Example (Python)

```python
import sys, json

first = True
for line in sys.stdin:
    req = json.loads(line.strip())
    if req.get("type") == "shutdown":
        break
    if req.get("type") != "poll":
        continue

    resp = {
        "status": "ok",
        "panels": [{
            "id": "hello",
            "label": "HELLO",
            "content": [{"key": "Msg", "value": "it works!", "style": "accent"}]
        }],
        "process_annotations": [],
        "metrics": []
    }

    if first:
        resp["manifest"] = {
            "name": "hello-plugin", "version": "0.1.0",
            "description": "Hello world plugin", "author": "you"
        }
        first = False

    print(json.dumps(resp), flush=True)
```

```toml
[[plugins]]
name = "hello"
command = "python"
args = ["path/to/hello_plugin.py"]
```

## Minimal Plugin Example (Rust)

The shared `dofek-plugin-protocol` crate provides serde-ready types for the
entire protocol — depend on it instead of redeclaring structs.

`Cargo.toml`:

```toml
[dependencies]
# From this workspace:
dofek-plugin-protocol = { path = "../../crates/dofek-plugin-protocol" }
# External plugins can use the published version once released:
# dofek-plugin-protocol = "0.1"
serde_json = "1"
```

`src/main.rs`:

```rust
use dofek_plugin_protocol::{Panel, PanelEntry, PollResponse};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines().flatten() {
        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match req.get("type").and_then(|v| v.as_str()) {
            Some("shutdown") => break,
            Some("poll") => {}
            _ => continue,
        }

        let mut resp = PollResponse::default();
        resp.status = "ok".into();
        resp.panels.push(Panel {
            id: "hello".into(),
            label: "HELLO".into(),
            content: vec![PanelEntry {
                key: "Msg".into(),
                value: "it works!".into(),
                style: "accent".into(),
            }],
        });

        writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap()).unwrap();
        stdout.flush().unwrap();
    }
}
```

## Tips

- **Always flush stdout** after writing a response — buffered output will cause timeouts
- **Use stderr for logging** — Dofek captures it, stdout is reserved for the protocol
- **Respond quickly** — you have `timeout_ms` (default 2s) to respond to each poll
- **Namespace your metric IDs** — use `plugin_name.metric_name` to avoid collisions
- **Ignore unknown fields** — Dofek may add new fields to poll requests in the future
- **Handle malformed input gracefully** — skip lines you can't parse, don't crash
- **Test standalone first** — pipe JSON manually before connecting to Dofek:
  ```bash
  echo '{"type":"poll","timestamp_ms":0,"processes":[]}' | python my_plugin.py
  ```
