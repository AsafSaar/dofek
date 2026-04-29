# dofek-docker

Dofek plugin that surfaces local Docker container status in the dashboard.

## What it shows

- **Plugin dock panel (`DOCKER`)** — up to three running container names with their image, plus an overflow count when more are running.
- **Ticker pill (`Containers`)** — live running-container count (`docker.containers` metric).
- **Process annotation** — tags `docker`/`containerd` processes with `category=dev` and the running-container count.

When the Docker daemon isn't reachable the panel shows `offline` in dim style and the rest of Dofek keeps running normally.

## Prerequisites

- Docker daemon reachable over HTTP. By default the plugin talks to `http://localhost:2375`.
  - **Linux**: enable the TCP socket in `/etc/docker/daemon.json` (`"hosts": ["tcp://127.0.0.1:2375", "fd://"]`) or run a side-car like [`socat`](http://www.dest-unreach.org/socat/) to bridge `/var/run/docker.sock` to TCP. Native Unix-socket support is on the roadmap.
  - **Windows**: in Docker Desktop → **Settings → General → "Expose daemon on tcp://localhost:2375 without TLS"**.
  - **macOS**: same as Windows for Docker Desktop.
- Rust toolchain (if building from source).

> ⚠️ Exposing the Docker daemon over plain TCP grants root-equivalent access to anything that can reach the port. Bind to `127.0.0.1` only, never to `0.0.0.0`.

## Build & install

From the repo root:

```bash
cargo build --release -p dofek-docker
```

Then copy the binary somewhere on your `PATH`:

| OS | Suggested install location |
|----|---|
| Linux / macOS | `cp target/release/dofek-docker ~/.local/bin/` |
| Windows | `copy target\release\dofek-docker.exe %LOCALAPPDATA%\Programs\dofek\` (and add that folder to `PATH`) |

## Configuration

Add this block to your `dofek.toml`:

```toml
[[plugins]]
name = "docker"
command = "dofek-docker"
args = ["--host", "http://localhost:2375"]
enabled = true
timeout_ms = 2000
```

### CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `http://localhost:2375` | Base URL of the Docker Engine HTTP API. `--url` is accepted as an alias. |

## Troubleshooting

- **Panel stuck on `offline`** — confirm `curl http://localhost:2375/containers/json` returns a JSON array. If your daemon listens on a different port or only on the Unix socket, adjust `--host` or expose the TCP endpoint.
- **No process annotation** — annotations match process names containing `docker` or `containerd`. On Docker Desktop the visible process is `com.docker.backend` (does not match by design — Desktop manages its own VM).
- **Plugin dock indicator is yellow** — five consecutive poll errors in a row. Check Dofek's stderr log for the underlying HTTP error.

## See also

- Plugin protocol reference: [`../README.md`](../README.md)
- Live JSON playground: <https://dofek.dev/plugins/>
- Source: [`src/main.rs`](src/main.rs)
