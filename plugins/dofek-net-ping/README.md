# dofek-net-ping

Dofek plugin that samples **TCP-connect latency** to a remote host and surfaces it in the dashboard.

Unlike `dofek-ollama` and `dofek-docker`, which poll an HTTP API on demand, this plugin runs a background sampler thread that pings *between* polls. The dashboard reads a stateful ring buffer of the last 60 samples — refresh rate is decoupled from ping rate.

## What it shows

- **Plugin dock panel (`NET-PING`)** — host:port, current latency, average/min/max over the buffer, and loss percentage.
- **Ticker pill (`Ping`)** — current latency in milliseconds (`netping.latency_ms` metric).

If the host stops responding the panel shows `timeout` and the loss counter ticks up; the rest of Dofek keeps running normally.

## Why TCP, not ICMP?

Raw-socket ICMP requires root / `CAP_NET_RAW` on Linux and elevation on Windows. TCP-connect time is what curl, your browser, and most apps actually see — and it works on every OS without privileges.

## Build & install

From the repo root:

```bash
cargo build --release -p dofek-net-ping
```

Then drop the binary somewhere on your `PATH`:

| OS | Suggested install location |
|----|---|
| Linux / macOS | `cp target/release/dofek-net-ping ~/.local/bin/` |
| Windows | `copy target\release\dofek-net-ping.exe %LOCALAPPDATA%\Programs\dofek\` (and add that folder to `PATH`) |

You can also point Dofek at the binary directly via an absolute path in `command`.

## Configuration

```toml
[[plugins]]
name = "net-ping"
command = "dofek-net-ping"
args = ["--host", "1.1.1.1", "--port", "443", "--interval-ms", "1000"]
enabled = true
timeout_ms = 2000
```

### CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `1.1.1.1` | Hostname or IP to TCP-connect to. |
| `--port` | `443` | Port to connect to. 443 is a safe choice — almost everything answers TLS. |
| `--interval-ms` | `1000` | Milliseconds between samples. Floored at `100`. `--interval` accepted as an alias. |

## Examples

Watch your home router:

```toml
args = ["--host", "192.168.1.1", "--port", "80", "--interval-ms", "500"]
```

Track latency to your production API:

```toml
args = ["--host", "api.example.com", "--port", "443", "--interval-ms", "2000"]
```

## Troubleshooting

- **Panel always shows `timeout`** — the host is blocking TCP on the chosen port. Try `--port 80` or any port you know answers (a firewall that drops 443 will drop ICMP too).
- **Loss percentage stuck at 100%** — DNS isn't resolving the host, or the network is down. Check `getent hosts <host>` (Linux/macOS) or `Resolve-DnsName <host>` (Windows).
- **Latency seems high vs. ICMP ping** — TCP connect adds roughly 1.5×RTT (SYN + SYN/ACK + ACK) plus the TLS handshake hint on port 443. That's expected; use it as a *trend* rather than an absolute reference.

## See also

- Plugin protocol reference: [`../README.md`](../README.md)
- Live JSON playground: <https://dofek.dev/plugins/>
- Source: [`src/main.rs`](src/main.rs)
