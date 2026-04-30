# v1.6 — Tabby integration (Option B: sidebar widget + dofek headless mode)

## Context

Tabby (https://github.com/Eugeny/tabby) is a TypeScript/Angular/Electron terminal emulator with ~70k stars, MIT-licensed, actively maintained. Its plugin ecosystem ships via npm (packages with the `tabby-plugin` keyword), but **no system-monitor plugin currently exists** — verified via Tabby's plugin registry search. Dofek's strength (live AI-aware system telemetry) maps cleanly onto that gap, and Tabby's reach (~50× dofek's installed base today) makes the integration strategically valuable.

The blocker: dofek has **no outbound integration surface**. The plugin protocol in `crates/dofek-plugin-protocol/src/lib.rs` is inbound only — Dofek hosts plugin children, not vice versa. There is no JSON stdout mode, no HTTP server, no headless flag (`src/config.rs:17-49`). `DataSnapshot` (`src/data/mod.rs:30`) is `pub Serialize` but only consumed by the TUI render loop and Tauri IPC (`gui/src/lib.rs`).

This plan adds a small `dofek-tui serve` headless mode (HTTP+SSE on localhost) and a separate npm package `tabby-dofek` (new repo at `AsafSaar/tabby-dofek`) that consumes it. The serve mode is reusable for any future integration (VSCode, Raycast, Emacs, MCP).

Status: **parked** — captured for a later cycle, not scheduled for the next release.

## Approach

Two deliverables in two repos.

### Part 1 — `dofek-tui serve` headless mode (this repo)

A new run mode that boots the data collector, skips the TUI, and exposes snapshots over a localhost HTTP server.

**CLI surface** (extend `src/config.rs:17-24`):

```
dofek-tui serve [--port <0|N>] [--token <secret>]
```

- `--port 0` (default) — bind to a random ephemeral port. Resolved port is written to `<config_dir>/dofek/serve.json` (alongside `plugins.toml`) so consumers discover it without hardcoding.
- `--port N` — bind explicitly to port N (for users who want predictability).
- `--token <s>` — optional shared secret. If set, requests must carry `Authorization: Bearer <s>`. Default is an auto-generated token written to `serve.json` (defense in depth — even though we bind to 127.0.0.1 only).

**Endpoints**:

- `GET /v1/snapshot` → current `DataSnapshot` as JSON.
- `GET /v1/stream` → Server-Sent Events; one `data:` frame per snapshot, at `general.refresh_ms` cadence.
- `GET /v1/health` → `{"ok": true, "version": "1.5.0"}`.

**Implementation**:

- New `src/serve.rs` module. Uses `tiny_http = "0.12"` (sync, ~1k LOC, no async runtime — matches dofek's existing sync architecture).
- Reuses the existing collector verbatim: `data::spawn_collector(config, refresh_ms)` (`src/data/mod.rs:79`) returns the `mpsc::Receiver<DataSnapshot>` we already use for the TUI. The serve mode just fans the receiver out to connected SSE clients via a broadcast `Arc<Mutex<Vec<Sender>>>`.
- `DataSnapshot` already derives `Serialize` (`src/data/mod.rs:29`). Need to remove `#[serde(skip)]` from `plugin_statuses` and decide a JSON shape for `Instant timestamp` (replace `#[serde(skip)]` with `#[serde(serialize_with = ...)]` emitting epoch-ms via `chrono::Utc::now()` — `chrono` is already a dep at `Cargo.toml:58`).
- Bind to `127.0.0.1` only. Hard-fail on attempts to bind elsewhere (no `--host` flag — minimize foot-gun surface).
- Wire `serve` as a new `CliCommand` variant alongside existing `Plugins`. `src/main.rs` dispatch: `match cli.command { Some(Serve { .. }) => serve::run(...), Some(Plugins ..) => ..., None => tui_main(...) }`.

**Files modified**:
- `src/config.rs` — add `Serve { port: u16, token: Option<String> }` to `CliCommand`.
- `src/main.rs` — dispatch the new subcommand before falling through to TUI init.
- `src/lib.rs` — `pub mod serve;`
- `src/serve.rs` — *new*, ~150 lines: tiny_http server, SSE broadcast, port/token discovery file.
- `src/data/mod.rs` — fix `DataSnapshot` serialization for `timestamp` and `plugin_statuses` (currently `#[serde(skip)]`).
- `Cargo.toml` — add `tiny_http = "0.12"` and `rand = "0.8"` (token generation).
- `dofek.toml.example`, `README.md`, `docs/manual.html` — document the new mode.

### Part 2 — `tabby-dofek` Tabby plugin (new repo `AsafSaar/tabby-dofek`)

Standalone npm-publishable Angular module following Tabby's plugin conventions (https://docs.tabby.sh/, https://github.com/Eugeny/tabby/blob/master/HACKING.md).

**Surface in Tabby**:

- **Custom tab type** — `DofekTabComponent extends BaseTabComponent` registered via `TabsService`. Renders a Canvas-based dashboard (CPU/GPU/MEM sparklines + process watchlist) styled to match Tabby's theme tokens. This is dofek's GUI feature-set, scoped down to fit a tab.
- **Toolbar button** — `ToolbarButtonProvider` adds a "Dofek" button that opens the tab (or focuses an existing one).
- **Config page** — `ConfigProvider` lets users override the `serve.json` discovery path or enter `host:port` + token explicitly (covers users running `dofek-tui serve --port 9876`).
- **Hotkey** — optional `Ctrl+Shift+M` to toggle the Dofek tab.

**Data layer**:
- On plugin init, read `serve.json` from dofek's config dir (`%APPDATA%\dofek\` on Windows, `~/.config/dofek/` on Linux, `~/Library/Application Support/dofek/` on macOS). If missing, surface a "dofek not running — `dofek-tui serve` to start" empty state with a copy-button.
- Consume `/v1/stream` via native `EventSource`. Re-render on each frame.
- On disconnect, exponential backoff retry; on 401, prompt for token in config.

**Repo structure** (`AsafSaar/tabby-dofek`):
```
tabby-dofek/
├── package.json              # name: "tabby-dofek", keywords: ["tabby-plugin"]
├── tsconfig.json
├── webpack.config.js         # mirrors official Tabby plugins (e.g. tabby-quick-cmds)
├── README.md                 # install + screenshots + config
├── src/
│   ├── index.ts              # NgModule with providers
│   ├── tab.component.ts      # DofekTabComponent
│   ├── tab.component.pug
│   ├── tab.component.scss
│   ├── toolbar.provider.ts   # ToolbarButtonProvider
│   ├── config.provider.ts    # ConfigProvider
│   ├── api/
│   │   └── client.ts         # SSE client + fetch wrapper
│   └── api/snapshot.types.ts # TypeScript port of DataSnapshot (codegen-friendly)
└── .github/workflows/
    └── publish.yml           # npm publish on tag
```

**Type-sync**: hand-write `snapshot.types.ts` initially. Stretch goal: generate from Rust via `ts-rs` once the schema stabilizes.

## Files to create / modify

### dofek (this repo)
- `src/serve.rs` *(new)* — tiny_http server + SSE broadcast.
- `src/lib.rs` — register `pub mod serve`.
- `src/config.rs:17-24` — add `Serve { port, token }` variant.
- `src/main.rs` — dispatch `Serve`.
- `src/data/mod.rs:29-44` — fix `DataSnapshot` serde skips for timestamp and plugin_statuses.
- `Cargo.toml` — add `tiny_http`, `rand`.
- `docs/manual.html`, `README.md`, `dofek.toml.example` — document `serve` mode (manual.html has three version-string spots per the project memory; none touched here, but the file's structure is the relevant docs target).

### tabby-dofek (new repo)
- All files listed in the repo-structure block above.

## Existing functions/utilities worth reusing

- `data::spawn_collector(config, refresh_ms)` (`src/data/mod.rs:79`) — the entire collector pipeline. Serve mode doesn't reimplement anything; it only fans the existing channel out to HTTP clients.
- `DataSnapshot` (`src/data/mod.rs:30`) — already `pub Serialize`. Just needs the two `#[serde(skip)]` fields fixed.
- `Config::load(cli)` (`src/config.rs:246`) — config-file discovery is identical to TUI mode; no duplication.
- `gui/src/lib.rs:22 get_snapshot()` — proves the serialization shape works end-to-end. The serve endpoint mirrors that contract.
- `dirs::config_dir()` (already pulled in via `Cargo.toml:51`) — for writing `serve.json` to the same dir as `plugins.toml`.
- For the Tabby side: model the npm package layout on `tabby-quick-cmds` (https://github.com/Eugeny/tabby/tree/master/tabby-plugin-template) — Tabby's official plugin template handles webpack + Angular wiring.

## Verification

1. **Headless mode boots**: `cargo run -- serve` exits cleanly on Ctrl-C. `cat ~/.config/dofek/serve.json` reveals port + token.
2. **Snapshot endpoint**: `curl -H "Authorization: Bearer $(jq -r .token ~/.config/dofek/serve.json)" http://127.0.0.1:$(jq -r .port ~/.config/dofek/serve.json)/v1/snapshot | jq .cpu.usage` returns a number; values change between calls.
3. **SSE stream**: `curl -N` against `/v1/stream` yields one `data:` frame per `refresh_ms`; SIGINT cleanly disconnects without panicking the server.
4. **Auth**: requests without the bearer token return `401`; requests from non-localhost are rejected (verify on Linux with `nc -v <host-lan-ip> <port>`).
5. **Tabby plugin loads**: in a Tabby dev install, `npm link` the plugin; restart Tabby; "Dofek" appears in the toolbar; clicking it opens a tab that renders live CPU/GPU/MEM sparklines.
6. **Resilience**: kill `dofek-tui serve` while the Tabby tab is open — the tab shows "dofek disconnected, retrying…" and recovers when serve is restarted.
7. **Cross-platform smoke**: same end-to-end check on macOS Apple Silicon and Windows 11.

## Out of scope (deliberately)

- **Reverse channel** (Option C): Tabby telling dofek the active tab's PID for a "TABBY" badge in the watchlist. Land Option B first; reassess after npm download numbers come back.
- **Embedding dofek-tui as a tab** (Option A): dropped; functionally equivalent to running `dofek-tui` in any tab and adds no value over Option B.
- **Bundling the dofek binary into the npm package**: keep the plugin a thin client. README points users at GitHub Releases. Bundling cross-platform binaries through npm is a packaging mess and ties plugin updates to dofek release cadence.
- **GUI parity**: the Tabby tab is a *summary* surface, not a clone of the Tauri GUI. No process kill, no settings editing, no NVML-detail panels in v1.
- **Auto-publish to npm registry**: scaffold the workflow but don't publish until we agree on a versioning scheme aligned with dofek's `dofek-version-bump` skill.
