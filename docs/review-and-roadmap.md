# Dofek — Deep Review & Extension Plan (v1.5.1 → v1.9)

## Context
Dofek v1.5.1 is a mature, solo-maintained OSS system monitor: ~9,900 lines of Rust, dual TUI (Ratatui) + GUI (Tauri 2) interfaces over one shared collector, a JSON-over-stdio plugin system with 3 first-party plugins, a website with a plugin playground, opt-in telemetry, and a Homebrew tap.

This document captures a deep review of architecture, features, capabilities, and security posture, followed by an executable roadmap spanning v1.5.2 → v1.9. Security fixes are rolled into normal releases (no CVE/advisory).

---

# Part 1 — Review

## 1.1 What's strong
- **Clean collection architecture.** Sync-only (no tokio), 3 threads + mpsc. One `spawn_collector` (`src/data/mod.rs:79`) reused verbatim by TUI and GUI — no duplicated collection logic. Refresh cadence in a shared `Arc<AtomicU64>` retunes the collector live without respawn.
- **Well-ordered sensor layering.** sysinfo (always) → Linux hwmon/RAPL fills CPU temp/power → NVML for GPU → LHM fills only remaining `None`s. Every degradation path is explicit and logged, never panics.
- **Disciplined platform abstraction.** Only 29 `#[cfg]` sites, all at genuine OS boundaries, unusually well-commented on *why*.
- **Sound unsafe usage.** 7 FFI sites (RtlGetVersion, OpenProcess/TerminateProcess, GetIfTable2), all correctly paired with their frees.
- **Good CI hygiene.** clippy `-D warnings` + tests on 3 OSes, scoped workflow permissions, release checksums, active Dependabot.
- Zero `TODO`/`FIXME`/`unimplemented!()` markers in the Rust sources.

## 1.2 Security findings (all verified directly in source)

| # | Finding | Evidence | Severity |
|---|---|---|---|
| 1 | **XSS → persistent RCE (GUI).** OS-controlled process names interpolated raw into `innerHTML` at `gui/frontend/index.html:1540`, plus `:1426, :1434, :1589, :1759, :1764` and a 7th sink at `~:1291` (`PLATFORM_INFO` from `sysctl`). No escape helper exists in the file. Amplifiers: CSP `script-src 'self' 'unsafe-inline'` (`tauri.conf.json:76`), `withGlobalTauri: true` (`:62`), and `plugins_add` (`gui/src/lib.rs:228`) which accepts any path with no Rust-side check — copies, `chmod +x`, strips macOS quarantine, spawns, persists to `plugins.toml`. Any local process naming itself `<img src=x onerror=…>` gets persistent code execution. | **High/Critical** |
| 2 | **CWD config → drive-by RCE.** `src/config.rs:250` searches `./dofek.toml` *first*. `store::resolve_command` (`store.rs:398-410`) returns unresolved names unchanged → `process.rs:26` `Command::new` resolves via CWD/PATH (CWD precedes PATH on Windows). `cd` into a hostile repo/zip and it executes on startup. | **High** |
| 3 | **Multibyte panic → local DoS (TUI).** `truncate` byte-slices a `&str` after a byte-length guard, in **6 copies** (`watchlist.rs:346`, `cpu.rs:151`, `network_disk.rs:99`, `bottom_strip.rs:431`, `gpu.rs:206`, `process_table.rs:430`). Any process with a CJK/emoji name at the wrong length panics the TUI. Not in the original brief; found and verified during design. | **High** (trivial fix) |
| 4 | **Plugin DoS.** Unbounded `read_line` (`process.rs:95`, `store.rs:330`) → OOM. Non-functional timeout (below) → collector freeze. Unbounded metrics in `ticker.rs:83-89` (no `.take()`, while `watchlist.rs:277` correctly bounds). | **Medium** |
| 5 | **`SECURITY.md` claims output size and rate limits are enforced. They are not.** | **Medium** (doc) |

Lower severity: LHM URL unvalidated + unbounded body (`src/data/lhm.rs:70-81`); no `cargo audit`/`deny` in CI across 686 transitive crates; 12 `Mutex::lock().unwrap()` on every GUI IPC handler (one panic bricks the GUI); symlink-following `fs::copy` in plugin install; PATH-resolved `xattr`/`sw_vers`/`sysctl`; stored-XSS path into the website admin dashboard from the unauthenticated public ingest endpoint.

## 1.3 The plugin runtime is the weakest subsystem
- **The poll timeout does not exist.** `read_line_timeout` (`process.rs:88-105`) does one *blocking* `read_line` and checks elapsed only in the `Err` branch. One wedged plugin freezes the collector — and therefore every metric in both UIs. `timeout_ms` is decorative. Same defect in `store::probe_manifest` (`store.rs:322-335`), which sits on the `plugins_add` IPC path and can park a Tauri IPC worker permanently.
- **stderr piped but never drained** (`process.rs:31`) → 64KB pipe deadlock. `plugins/README.md:24` claims otherwise.
- **Plugins polled sequentially** on the collector thread → latency stacks (3 plugins × 2s timeout = 6s stall).
- **No Job Objects** despite the `CLAUDE.md:88` claim — only `CREATE_NO_WINDOW`. Children orphan on force-kill.
- `PollResponse.status` parsed, never read. `schema_version` documented in **5 places**, exists nowhere in code.
- **The GUI plugin dock is dead UI.** `plugin_statuses` is `#[serde(skip)]` (`data/mod.rs:42`), so `index.html:428-437` is static "PLUGINS 0 active" HTML. You can install plugins in the GUI but never see their output. `ProcessInfo.plugin_label` is set and never rendered anywhere.

## 1.4 Other structural gaps
- **Tests are effectively absent** — 4 unit tests (`src/update.rs`) for ~9,900 lines. CI runs `cargo test` on 3 OSes and validates almost nothing.
- **Everything ships unsigned** — SmartScreen/Gatekeeper friction documented in 5 places. No auto-update. Plugins bundled in no installer except the Homebrew formula.
- **Zero outbound integration surface** — no JSON stdout, no HTTP API, no MCP. For a self-described "AI-aware" monitor, nothing can query it.
- macOS shows N/A for GPU/VRAM/temp/power; AMD unsupported.
- Net sparklines splice different NICs (`app.rs:257` takes `interfaces.first()` from a traffic-sorted list); `app.rs` is 953 lines with a ~240-line `handle_key`; `#![allow(dead_code)]` blankets the TUI binary.

---

# Part 2 — Release Plan

## Guiding sequencing constraints
1. **Test scaffolding lands before the rewrites** — there is no net today.
2. **The XSS fix must precede un-skipping `plugin_statuses`.** That `#[serde(skip)]` is currently the only thing keeping plugin-controlled strings out of the webview. Both the GUI dock (v1.7) and `serve`/MCP (v1.8) need it removed — so PR 2 is a hard prerequisite, not a nice-to-have.
3. **Signing must precede auto-update** — shipping unsigned MSIs through an updater re-triggers SmartScreen on every update.
4. **Start the paperwork now** — SignPath and Apple Developer enrollment have multi-week latency and zero code cost.

---

## v1.5.2 — Security release (PRs 1–3)

### PR 1 — Test scaffolding + pure-function tests + two bug fixes ✅ *implemented*
Establishes the net, and fixes finding #3 on the way.

> **As-built notes (2026-08-01).** 99 tests, all green (up from 4), plus `clippy --workspace --all-targets --all-features -D warnings` clean. Deltas from the plan below:
> - **Added beyond plan: `src/ui/mod.rs` headless render tests** via ratatui's `TestBackend` — every view (2 focus modes × 5 chart tabs × 4 category filters) at 7 terminal sizes from 200×60 down to 12×8, with CJK / emoji / RTL / combining-mark names at every character length. This is what actually proves finding #3 is fixed *in situ*, rather than only at the helper. Verified by reverting `truncate` to the byte-slicing version: 3 of the 5 render tests fail with the mid-codepoint panic; with the fix, all pass.
> - `truncate` counts **characters, not display columns**. That closes the panic (finding #3, reproduced first: `&"字幕処理サービス"[..8]` panics mid-codepoint). A CJK name still occupies two cells per character, so it can overflow its column visually — a layout imperfection, not a crash, deliberately left rather than pulling in `unicode-width`. Recorded in the module doc comment.
> - Two helpers, not one: `truncate` (`...`) and `truncate_with` (arbitrary marker), because `watchlist.rs` used a two-dot marker to save a column and that's worth preserving.
> - `rate.rs` also exports `rate_from_delta(delta, elapsed_s)` for callers computing their own delta. `rapl.rs` uses it for the µJ→J conversion; it keeps its own `elapsed <= 0.0 ⇒ None` guard, since "no reading yet" and "0 W" are different facts.
> - **New finding, pinned by test:** `resolve_command_in(dir, "")` resolves to the plugins *directory*, because `dir.join("")` is `dir` and the check is `.exists()`, not `.is_file()`. Harmless today (it fails at spawn instead of at resolve), but **PR 3 must require a regular file** — added to that scope below. Left unfixed here to keep this PR behaviour-preserving; `empty_command_currently_resolves_to_the_plugins_dir` documents it and will flip when PR 3 lands.
> - `tests/mock_plugin_smoke.rs` covers the fixture's own behaviour so it can't rot before PR 4 consumes it. `--hang`, `--slow`, `--stderr-flood`, and `--fork-grandchild` are **not** smoke-tested: against today's blocking reader they would hang the suite. They are PR 4's regression tests, by construction.
> - CI now runs `--all-features` for both clippy and test on all three OSes (needed for the `mock-plugin` gate).
> - `.tmp/` added to `.gitignore`.

- **`src/ui/text.rs` (new)** — one char-boundary-safe `truncate` using `char_indices`; delete the 6 duplicate copies. Model on `update.rs:84`'s `truncate_notes`, which already does this correctly. Test with emoji/CJK.
- **`src/data/rate.rs` (new)** — extract `rate(cur, prev, elapsed_s) -> f64` shared by `network.rs:75`, `disk.rs:66`, `rapl.rs`; guard `elapsed_s <= 0.0 => 0.0` (today it can yield `inf`, which `serde_json` serializes as `null` into the GUI).
- **Refactor-for-testability extractions** (no behavior change): `resolve_command_in(dir, cmd)` out of `store.rs`; `pick_temp_from(iter)` out of `sysinfo_source::pick_cpu_temp`; `group_rows(...)` as a free fn out of `App::grouped_rows` (so tests don't need a `TelemetryHandle`).
- **`src/bin/dofek-mock-plugin.rs` (new)** behind a default-off `mock-plugin` feature; integration tests reach it via `env!("CARGO_BIN_EXE_dofek-mock-plugin")`. Modes: `--hang`, `--slow`, `--crash-after`, `--flood`, `--giant-line`, `--stderr-flood`, `--fork-grandchild`, `--legacy`, `--status-error`. Add `tempfile` dev-dep. CI test command gains `--all-features`.
- **Unit tests** for `ai_detect::classify_process` (table test across all branches), `lhm::parse_lhm_value` + `find_path` + a 10k-deep-JSON test asserting `Err` not overflow, `pick_temp_from`, `rate`, `SparklineBuf`/`CandleBuf`, `group_rows`, `truncate`.

### PR 2 — Frontend XSS + CSP *(prerequisite for v1.7 and v1.8)* ✅ *implemented*

> **As-built notes (2026-08-01).** Finding #1 reproduced first, against the real `createProcRow`/`createGroupRow` templates: 3/3 payloads (`<img onerror>`, `<script>`, `title=` attribute breakout) reached the DOM raw. After the fix all are inert, verified by loading the shipped `esc()` out of `app.js` rather than reimplementing it. Deltas from the plan:
> - **Three files, not two:** `app.css`, `app.js`, `overlays.js`. The second inline `<script>` binds listeners to overlay markup that appears *after* the first script, so merging them would change execution order. Each `<script src>` sits exactly where its inline block was. Extraction verified line-by-line as verbatim against `git HEAD`.
> - **`delete window.__TAURI__` needed restructuring to be safe.** Three sites re-read the global after boot (`tick()`, `startSnapshotListener()`) because it isn't always populated at parse time — deleting at boot would have broken IPC acquisition in exactly the race those retries exist for. Replaced with one `acquireTauriApi()` that deletes only once *both* `invoke` and `listen` are held.
> - **8 sinks, not 7** — the plan missed `title="Kill all ${g.name}"` in `createGroupRow`, a second injection in the same template and an *attribute* context (a name containing `"` closes the attribute and opens an event handler). Now `setAttribute`. Group name also became a text node rather than a nested span, so the DOM is byte-identical to before.
> - Numeric/literal sinks were annotated `// SAFE:` rather than changed, so the guard test can be strict without churning correct code.
> - The guard test needed a small block-comment scanner: app.js's row-cache commentary spans several lines and discusses `innerHTML` in prose. It also scopes the raw-name check to lines that actually open an HTML tag, so `sigFor`'s cache-key template and `setAttribute` don't trip it.
> - **`SECURITY.md`: corrected finding #5 rather than leaving it for v1.6.** The file claimed output size and rate limits were enforced; they are not. Now states plainly that they aren't yet, and why it's a robustness gap rather than a privilege boundary. PR 4 makes the original claim true and can restore it.
> - **Runtime verification** used temporary `log::info!` probes in the IPC handlers (since removed; `gui/src/lib.rs` is unmodified). Under `script-src 'self'` the frontend boots and calls `get_app_version`, `get_settings`, `get_telemetry_prompted`, `get_snapshot`, `get_platform_info`; `overlays.js` executes and shares scope; `__TAURI_INTERNALS__` stays reachable for the dialog/shell plugins. `get_snapshot` fires **once**, not once per second — proof `tauriListen` was acquired and the event path survived the `__TAURI__` deletion, since the failure mode is a 1 Hz polling fallback.
> - **Not done:** the end-to-end run with a real hostile-named process. Copied system binaries are killed by macOS signature checks, and the script-based retry was declined. The gap is narrow — names now go through `textContent`/`setAttribute`, so there is no HTML parse to exercise — but it is untested at the process level.
> - Note for whoever builds next: Tauri **embeds** `frontendDist` into the binary at compile time. Editing a frontend file without rebuilding runs the *previous* frontend, which silently invalidates any manual GUI check.

- Extract the inline `<script>` to `gui/frontend/app.js` and `<style>` to `app.css`; set **`script-src 'self'`** in `gui/tauri.conf.json:76`. This is the actual fix — it downgrades the chain from persistent RCE to cosmetic DOM injection. Keep `'unsafe-inline'` for `style-src` only (static `style="…"` attributes; CSSOM assignment isn't CSP-governed anyway) and record the residual in SECURITY.md.
- Add `esc()` and apply at all **7** sinks; use `textContent` for the two hot paths (`createProcRow`, `createGroupRow` — set `.pname` after `innerHTML` builds the row; the reconciler never rewrites names, so there's no measurable cost on the 500-row path).
- Keep `withGlobalTauri: true` (removing it breaks the bundler-free frontend). Instead capture `invoke`/`listen` at boot then `delete window.__TAURI__` — defense-in-depth only, and smoke-test against the dialog/shell plugins which call through `__TAURI_INTERNALS__` (`index.html:675`).
- **`tests/frontend_no_raw_innerhtml.rs` (new)** — fails on any `innerHTML` not preceded by a `// SAFE:` comment, and asserts `script-src` contains no `'unsafe-inline'`. Makes the guarantee enforceable rather than aspirational.

### PR 3 — Binary resolution + config provenance ✅ *implemented*

> **As-built notes (2026-08-01).** 132 tests green, `clippy --workspace --all-targets --all-features -D warnings` clean, and `cargo deny check advisories bans licenses sources` passes — **no known advisories across the tree**. Deltas from the plan:
> - **Finding #2 was pinned by test rather than by live exploit.** Two attempts to stage a hostile-CWD repro were declined by the sandbox, so rather than work around that, the fix is guarded by `cwd_config_is_never_a_candidate`, which asserts no candidate path is even *relative* — a stronger and permanent invariant than a one-off marker file. The chain itself was already verified by inspection.
> - `config_candidates(cli, env, config_dir)` extracted so the search order is testable without touching the process's CWD or environment.
> - **`resolve_command` rejects relative paths outright** rather than joining them onto the plugins dir. `../../../bin/sh` would otherwise escape it, and a relative command in a config file is exactly the CWD-dependence being removed. Symlink containment is tested both ways: a link out of the managed dir is refused, a link within it still works.
> - The empty-command case PR 1 pinned is now an error, and that test flipped as planned.
> - **`store::add` reordered**, not just hardened: `create_new(true)` means the "already installed" case now fails *before* the copy instead of after, so it needed its own error message. The duplicate-by-display-name check still runs later, since the manifest name isn't known until the binary is probed.
> - `lock_or_recover` applied to **11** sites, not 12 — 7 IPC handlers plus the 4 in the setup/relay threads, which have the same failure mode.
> - **Config validation added** (`Config::validate`): a bad `lhm.url` degrades to the default with a warning rather than failing the load, since a monitor shouldn't refuse to start over an optional sensor source.
> - `deny.toml` pins `[graph].targets` to the three shipped platforms — without it cargo-deny walks trees for targets that never reach a binary. `multiple-versions = "warn"`, since Tauri and ratatui legitimately pull overlapping ecosystems (currently 2 duplicates: `thiserror`, `thiserror-impl`).
> - Verified end-to-end through the CLI against an isolated `HOME`: install → list → on-disk → reinstall refused → directory refused → installed plugin resolves and speaks the protocol → remove.
> - Note for whoever builds next: do **not** `export HOME` and then run `cargo` in the same shell. Cargo picks up `$HOME/.cargo` as `CARGO_HOME`, re-resolves from scratch, and drops the top-level binary hardlinks in `target/debug/`. This looked exactly like a crash for several minutes.

- **`src/config.rs:250`** — drop `./dofek.toml` from the default search order. New order: `--config` → `$DOFEK_CONFIG` → `dirs::config_dir()/dofek/dofek.toml`. The env var preserves the project-local workflow explicitly.
- **`store::resolve_command` → `Result<PathBuf>`.** Returning the string unchanged *is* the vulnerability. Rules: absolute + canonicalized + regular file ⇒ Ok; relative-with-separator ⇒ Err; bare name ⇒ join onto `plugins_dir`, canonicalize, verify still inside `plugins_dir` (defeats a planted symlink) ⇒ Ok; else Err. **Never PATH, never CWD.** Note the existence check must be `is_file()`, not `exists()` — PR 1's `empty_command_currently_resolves_to_the_plugins_dir` test pins the current behaviour where an empty command resolves to the plugins directory itself; flip that test here.
- **`plugins_add` provenance gate** — path validation cannot help here (any local binary is a valid plugin path). Add `pending_plugin_paths: Mutex<Vec<(PathBuf, Instant)>>` to `AppState`; `plugins_pick_file` records the picked path, `plugins_add` rejects anything not present, 5-minute expiry, consumed on use. The `dofek-tui plugins add` CLI stays unrestricted — a terminal invocation is the user's intent. Add a GUI confirm modal stating plugins run with full user privileges.
- **`store::add` hardening** — open the source once and validate on the *handle* (regular file, ≤256 MiB), `io::copy` into a dest opened `create_new(true)`. Eliminates both the source TOCTOU and a pre-planted destination symlink — strictly better than the `fs::copy` at `store.rs:166`. Sanitize `file_name` to `[A-Za-z0-9._-]`; bound `args` (32 max, 1024 bytes each, no NUL/control chars).
- Absolute paths for `/usr/bin/xattr`, `/usr/bin/sw_vers`, `/usr/sbin/sysctl`.
- Update the docs that describe `command` as PATH-resolved (`plugins/README.md:39`, README, website, manual).

**Also in this release (low-risk hygiene):** `lock_or_recover` helper replacing 12 `lock().unwrap()` in `gui/src/lib.rs` (preserve the lock-ordering comment at `:113-115` — that analysis is still load-bearing); LHM body cap (8 MiB via `into_reader().take(...)`) + URL validation at `Config::load` (reject control chars, require http/https, warn on non-loopback); `cargo-deny` CI job with a weekly cron so advisories surface without a code change.

---

## v1.6 — Plugin runtime rework + signing

### PR 4 — Plugin runtime ✅ *implemented*

> **As-built notes (2026-08-01).** 169 passing across the workspace (56 lib + 80 TUI-bin + 6 + 7 + **20 new** in `tests/plugin_runtime.rs`), 1 ignored (the pre-existing `update::tests::live_check`). `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean; `cargo deny check advisories bans licenses sources` ok. Deltas from the plan below:
>
> - **Findings reproduced first, against the pre-v1.6 primitive.** A standalone binary replicating `read_line_timeout` verbatim was compiled in the scratchpad and run against the fixture: `--hang` blocks forever with `timeout_ms=500` (killed at 8 s), and `--stderr-flood` deadlocks before the plugin can write a single byte to stdout. Orphaning was reproduced separately: kill a `--fork-grandchild` plugin and its child is still alive afterwards. All three now pass as tests.
> - **New: `CollectorHandle`, and `spawn_collector` returns a tuple.** Not in the plan, but `setsid` *created* an obligation — a plugin in its own session no longer receives the terminal's job-control signals, so on quit every plugin would have been orphaned. Nothing called `PluginManager::shutdown` before (the collector thread is detached, so its destructors never ran). Both frontends now stop the collector explicitly: TUI before the terminal restore, GUI on `RunEvent::Exit`. The inter-tick `thread::sleep` became `recv_timeout` so shutdown doesn't wait out a full refresh interval. `collector_delivers_plugin_data_and_stops_the_child_on_shutdown` covers the whole path end to end.
> - **`is_alive()` is gone, deliberately.** `Child::try_wait` *reaps*, and a reaped pid is free for the OS to reissue — which is exactly the process **group** id we are about to `killpg`. Nothing reaps outside `PluginProcess::kill`, guarded by a `reaped` flag, and every signal goes out before the reap. Death is detected from the reader thread's `Closed` event instead, which is equivalent and arrives just as fast. Same reason `shutdown_and_kill` uses a flat 150 ms grace rather than a polled one: the obvious "stop early when it exits" optimisation is what introduces the race.
> - **The group id is captured once at spawn**, after a `getpgid` check that `setsid` actually took. If it didn't, only the child is signalled — `killpg` against our *own* group would signal dofek itself.
> - **Group teardown runs even when the plugin exits cleanly.** The first draft returned early on a graceful exit, which made `grandchild_dies_with_plugin` fail: a plugin that quits leaving helpers behind is precisely the orphan case containment exists for.
> - **`FLOOD_LINES` is 3, not the 8 first written.** `sync_channel(RESPONSE_QUEUE)` caps the reader at 4 queued lines, so a threshold of 8 could never fire — the backpressure that bounds memory also bounds what a drain can observe. Three consecutive near-full drains now disconnect the plugin.
> - **`Outcome::Stopped`** — `await_response` slices its wait and checks the stop flag, otherwise shutting down mid-poll cost that plugin's full `timeout_ms` (2 s default, unbounded in principle since it is user config). This was a real failure: `shutdown_is_prompt` and `replace_swaps_the_plugin_set_promptly` both measured ~2 s before it.
> - **`PollRequestRef`** (a borrowing mirror of `PollRequest`) removes the per-plugin `to_vec()` without hand-rolling JSON. `request_wire_format_matches_the_owned_type` asserts the two serialize identically, so the optimisation can't silently drift from the documented shape.
> - **Windows containment is assign-after-spawn, not `CREATE_SUSPENDED`.** `std::process::Command` hands back no main-thread handle, so resuming needs a toolhelp snapshot; `PROC_THREAD_ATTRIBUTE_JOB_LIST` needs a hand-rolled `CreateProcessW` with manual pipe setup. **There is no Windows toolchain on this machine** (homebrew rust, no `rustup`), so either version ships unverified — and the failure mode of a botched resume is a permanently suspended plugin on every Windows install. Chose the version that cannot strand a child. The residual (a fork in the microseconds before assignment escapes the job) is recorded in the module doc and in `SECURITY.md`. **This needs a real Windows check before release.**
> - **`store::probe_manifest` is now `pub`** so it can be tested directly rather than only through a real install — it sits on the `plugins_add` IPC path, where the same fake timeout could park a Tauri worker permanently. `probe_of_a_silent_binary_times_out` is that regression test. `#[allow(clippy::never_loop)]` is gone.
> - Probe results are sanitized too (`clean_short`/`clean_long`): they are written into `plugins.toml` and shown in both UIs without passing through `sanitize_response`.
> - **Not verified live:** `dofek-tui plugins add` against an isolated `HOME`. Three attempts were SIGKILLed by the sandbox at exit 137, twice taking the `dofek-tui` binary with them. Every step it performs is covered by tests — `probe_manifest` directly, `store::add`'s validation by unit tests, `resolve_command` from PR 3 — but the CLI wrapper itself was not exercised end to end. Same class of blocker as PR 2's and PR 3's.
> - Docs made true rather than corrected: `schema_version: 1` now exists (5 sites), `plugins/README.md`'s stderr claim now holds, `CLAUDE.md`'s Job Object claim now holds, and `SECURITY.md`'s output-limit claim — which PR 2 had to walk back — is restored with the actual numbers.
> - `ticker.rs` gained the belt-and-braces `.take(6)` the plan called for.

**Design decision: one supervisor thread per plugin, not reader-thread + `recv_timeout` on the collector.** The latter fixes the fake timeout but leaves sequential-polling latency stacking and keeps `shutdown()`'s 2s and `replace()`'s 200ms sleeps on the collector. Plugin counts are single-digit; 3 threads each is fine and matches how this codebase already works.

Per plugin: **supervisor** (owns `Child`/`ChildStdin`/backoff, `recv_timeout`, sleeps in 50ms slices so join is bounded), **stdout reader** (bounded `read_line` → `sync_channel(4)`, sacrificial — dies when the supervisor kills the child), **stderr drainer** (bounded reads → rate-limited `log::debug!`, fixes the pipe deadlock).

- **`src/plugin/worker.rs` (new)** — `PluginWorker { status: Arc<Mutex<PluginStatus>>, stop: Arc<AtomicBool>, join }`. Shared input context as one `Arc<Vec<ProcessContext>>` swapped per tick (removes today's per-plugin `to_vec()` amplification). Bounded reads via `reader.by_ref().take(256 * 1024).read_line(...)`; over-long ⇒ disconnect, not resync.
- **`src/plugin/sanitize.rs` (new)** — cap at **ingest**, not at render: 8 panels, 16 entries/panel, 8 metrics, 512 annotations, strings 64/128 chars, char-boundary-safe truncation, control chars and `\r\n` stripped. This is what makes the `SECURITY.md` claim true. Render-site `.take()` in `ticker.rs` stays as belt-and-braces (a one-line ticker breaks on unbounded pills even at legal sizes).
- **`src/plugin/process.rs` rewrite** — delete `read_line_timeout`. Add containment: Windows `CreateJobObjectW` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` + process/memory limits, spawned `CREATE_SUSPENDED` then resumed so assignment wins the race against a forking child (needs `Win32_System_JobObjects` feature); Unix `pre_exec` → `setsid()`, kill via `killpg` after a SIGTERM grace (needs `nix` `process` feature). Makes the `CLAUDE.md:88` claim true.
- **`PluginManager::tick(&self, ...)` — non-blocking**, two mutex ops. Skip building `Vec<ProcessContext>` entirely when there are no plugins (today `data/mod.rs:227-234` clones ~500 `String`s/sec on every install regardless).
- **`store::probe_manifest`** rewritten on the same primitive; make `plugins_add` async + `spawn_blocking` (mirroring `check_for_update` at `gui/src/lib.rs:61`). The `#[allow(clippy::never_loop)]` disappears — CI will confirm.
- **`tests/plugin_runtime.rs` (new)** — one regression test per finding: `hung_plugin_does_not_stall_collector` (10 ticks each <50ms), `three_plugins_do_not_stack_latency`, `stderr_flood_does_not_deadlock` (10MB), `giant_line_disconnects_plugin`, `flood_is_rate_limited`, `crash_respawns_with_backoff`, `shutdown_is_prompt` (<500ms vs 2s today), `grandchild_dies_with_plugin` (unix), `legacy_plugin_still_works`, `status_error_marks_unhealthy`.
- **Protocol** (`crates/dofek-plugin-protocol`): add `schema_version` and `seq` to both request and response, all `#[serde(default)]` so the 3 first-party and any third-party plugins keep working. Start honoring `status == "error"` ⇒ `Unhealthy`. Then correct the 5 doc sites that already claim `schema_version` exists.

### Signing + notarization (parallel, mostly ops) 📋 *runbook written — see [`docs/v1.6-signing-and-notarization.md`](v1.6-signing-and-notarization.md)*

> **As-built notes (2026-08-01).** Both applications are prepared but neither is submitted — both need a human identity and Apple needs a payment method. Deltas from the plan below:
>
> - **"Zero code" was not quite true.** SignPath's terms require product name and version metadata set consistently across all signed binaries. There is no `build.rs` anywhere in the workspace and no `winres`/`winresource` dependency, so `dofek-tui.exe` and the three plugin exes ship with **no VERSIONINFO at all** — only the Tauri-built GUI exe and MSI carry it. A reviewer sees this in one right-click on a current release artifact. Four small build scripts are a prerequisite for applying, not a follow-up.
> - **A second prerequisite: the code signing policy page.** The terms require it on the homepage *and* download pages, with roles, privacy policy, and SignPath attribution — and the application form asks for its URL. Full draft is in Appendix A of the runbook; it needs to be live at `dofek.dev/code-signing-policy` before submitting.
> - **The Azure fallback is narrower than the plan assumed.** Trusted Signing became Azure **Artifact Signing** and reached GA in April 2026 with a hard split: **individual** sign-up is United States and Canada only, while **organizations** get a 12-country list (incl. EU/UK/Israel). The 3-years-of-history requirement is gone. For an individual outside the US/Canada there is no fallback here at all — it becomes "enroll a legal entity" or "buy a conventional OV cert" (~$200–400/yr, hardware token, SmartScreen reputation from zero). Confirm which bucket applies before treating this as a real backstop.
> - **The kill capability has to be disclosed, not glossed.** The terms bar undisclosed privacy-affecting behaviour, and dofek terminates processes, reads the full process list, and runs user-installed plugin binaries. The runbook carries a written disclosure paragraph covering all of it — better to volunteer it than to have a reviewer find it.
> - **Individual over Organization for Apple**, recommended: 24–48 h vs 1–4 weeks of D-U-N-S paperwork, and the publisher string is already "Asaf Saar" in `bundle.publisher` and the copyright.
> - **Developer ID *Installer* is not needed** — dofek ships `.dmg` + `.app.zip`, never a `.pkg`.
> - **App Store Connect API key over an app-specific password** for `notarytool`: not coupled to Apple ID 2FA, survives a password change, independently revocable.
> - **The GUI path needs no workflow change at all** — Tauri signs the `.app`, the sidecar, notarizes and staples the `.dmg` from the `APPLE_*` env vars alone. Only the four standalone Mach-O downloads need explicit `codesign` + `notarytool` steps, and they cannot be stapled (bare Mach-O), so first launch checks online.
> - Exact insertion points recorded against current line numbers: `release.yml:65→99` (Windows, before checksums) and `release.yml:281→327` (macOS). The doc-cleanup list is **7 sites, not 5** — `README-install.txt:23-25` and the release-notes template at `release.yml:485` were missed in the original count.

- **Apply now, zero code:** SignPath Foundation OSS program (**$0**, multi-week approval, cert reads "SignPath Foundation") and Apple Developer Program (**$99/yr**, unavoidable, enroll early). Fallback if SignPath doesn't work out: Azure Trusted Signing ~$120/yr — but **verify country eligibility first**, individual signup is public-preview with a limited country list.
- Windows: sign MSI + `dofek-tui.exe` + 3 plugin exes. `release.yml` lines 65-97 already collect them in one directory — insert signing *before* checksum generation so SHA256SUMS covers signed bytes.
- macOS: Tauri handles the `.app`/`.dmg` chain via `APPLE_*` env vars and signs the sidecar automatically. Standalone TUI + plugins need their own `codesign --options runtime` + `notarytool submit --wait` steps (bare Mach-O can't be stapled; online check on first run is fine).
- **Ordering trap:** the updater's minisign `.sig` is computed over final bytes. With SignPath's post-build pipeline the MSI changes *after* bundling — updater artifacts must be re-signed with `cargo tauri signer sign` afterward.
- Then delete the `xattr -dr` instructions from all 5 doc locations. Keep `store::clear_quarantine()` — third-party plugins stay unsigned.
- Free win: GPG-sign the combined `SHA256SUMS.txt`.

---

## v1.7 — Plugin surfacing + auto-update + bundling

### PR 5 — Plugin surfacing *(depends on PR 2 and PR 4)* ✅ *implemented*

> **As-built notes (2026-08-01).** 188 passing across the workspace (60 lib + 92 TUI-bin + **9** frontend guards + 7 + 20), 1 ignored (the pre-existing `update::tests::live_check`) — **15 new**. `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean; `cargo deny check advisories bans licenses sources` ok. Deltas from the plan below:
>
> - **`PluginState` serializes lowercase**, not as the Rust variant spelling. The GUI keys its dot colours off the string, and the enum already had a `Display` impl producing `healthy`/`crashed`/…; two spellings for one concept is a bug waiting to happen. `plugin_state_serializes_lowercase` asserts the wire form and the `Display` impl can never drift apart.
> - **The TUI dock's height is now content-driven with a *budget*, not a cap.** `.min(6)` was a cap on content; the replacement computes every line the dock wants (`plugin_dock_lines`) and clamps it against `inner.height / 3` collapsed, `inner.height - 6` expanded. `MIN_TABLE_ROWS` is a named constant used both as the layout `Constraint::Min` and as the expanded budget's reserve, so the two can't disagree.
> - **Overflow is stated, not silent.** When the lines don't fit, the last visible row becomes `+N more — P to expand`. The dock is the only place a plugin's output surfaces, so truncation had to be discoverable; the hint drops the key suffix once already expanded.
> - **First panel inline, rest on their own lines.** Rendering every panel on its own line would have doubled the dock's height for the single-panel plugin that is the common case. Inlining the first panel keeps that at one row — the density the v1.5 dock had — while everything else is still reachable.
> - **Entries render `key value`, not just `value`.** The old dock showed two bare values with no indication of what they were. `sanitize` caps keys at 64 chars, so this costs nothing.
> - **`P`, not a new lowercase key.** Lowercase `p` is the full-screen process view; `X` already set the precedent for a shifted variant. It falls through the `PanelFocus::Processes` branch to the global handler, so it works from either view.
> - **The name cell became a `Line` of spans, so `cells` is now `Vec<Cell>`.** A `Vec<Span>` can't carry a two-style cell. The label is budgeted *from within* the name column (at most a third of it, never more than the label needs) and dropped entirely below `MIN_NAME_WITH_LABEL` — the process name keeps priority, because an unidentifiable row is worse than a missing annotation.
> - **The GUI dock renderer is held to a stricter standard than the rest of the frontend.** `every_html_sink_is_annotated_safe` lets a `// SAFE:` comment justify an `innerHTML`; that is not good enough for a region rendering third-party binaries' output. `plugin_dock_renderer_has_no_html_sinks_at_all` bans `innerHTML`/`insertAdjacentHTML`/`outerHTML`/`document.write` outright inside explicit `PLUGIN DOCK` … `END PLUGIN DOCK` markers, so there is no line for a future edit to add an interpolation to. The guard skips comments (the section's own prose names the sink it avoids) and was **verified by injecting a violation** — it flagged the exact line, and passed again once reverted.
> - **The GUI dock was verified headlessly**, not just by inspection: a Node DOM stub whose `innerHTML` setter throws, driven with a plugin whose `display_name` is `<img src=x onerror=alert(1)>` and whose panel value is `</span><script>bad()</script>`. Both land as literal `textContent`; an unknown `style` falls through to the neutral class; the dot/state/count all resolve. This matters because Tauri **embeds** `frontendDist` at compile time, so eyeballing the running GUI without a rebuild tests the previous UI.
> - **`upPlugins` is signature-gated.** A plugin reporting a static panel is the common case, and a blind rebuild each second would also throw away the user's scroll position in a scrollable dock. `pdSig` short-circuits; both the reuse and the rebuild paths are covered by the headless check.
> - **`_PD_STATE_CLS` / `_PD_ENTRY_CLS` are whitelists.** Values go in via `className`, so this isn't an injection fix — it stops a plugin putting arbitrary tokens into a class attribute and stumbling onto an unrelated rule.
> - **Docs corrected where they had gone stale**: `plugins/README.md` and the website plugin page both claimed only the first panel was rendered inline (true before, misleading now); the annotation table said `label` is "displayed as `plugin_label`", which described a field that was never drawn. `P` added to `README.md`, `docs/manual.html`, `CLAUDE.md`, the help overlay, and the status bar.
> - **No CHANGELOG entry**, matching PRs 1–4 — the working tree has no `[Unreleased]` section and inventing one for a single PR would be inconsistent. These as-built blocks are the record until a version bump.

- Derive `Serialize` on `PluginStatus` and `PluginState`; **remove `#[serde(skip)]` from `plugin_statuses`** (`data/mod.rs:42`). Safe now: PR 2 closed the XSS sinks and PR 4's sanitizer bounds the payload to a few KB.
- **GUI dock** — replace the static markup at `index.html:428-437` with a real renderer built from `createElement` + `textContent` only. Scrollable rather than truncated.
- **TUI dock** (`watchlist.rs`) — replace the hard `.min(6)` and `panels.first()`/`.take(2)` with all panels and entries, clamped to `inner.height / 3` so it can't starve the process table. Add a key to expand/collapse full-height.
- **`plugin_label`** finally rendered: TUI dim suffix in the name cell; GUI badge **via `textContent`** — this is the sink that makes PR 2 non-optional.

### Auto-update + plugin bundling ◐ *partially implemented — the updater itself is deliberately still open*

> **As-built (2026-08-01).** 208 passing across the workspace (73 lib + 99 TUI-bin + 9 frontend + 7 + 20), 1 ignored — **+20 since PR 5**. Clippy clean, `acl-manifests.json` reverted. Split by what sequencing constraint 3 allows:
>
> **Done — plugin bundling (not gated on signing):**
>
> - `externalBin` now carries all four binaries; `prep-sidecar.{sh,ps1}` build and stage each with its triple suffix, and fail loudly if one is missing rather than letting Tauri report an opaque "resource path doesn't exist". No `build-all` change needed — those call `cargo tauri build`, so the `beforeBuildCommand` hook already fires.
> - **`resolve_command`'s bundled probe is a closed allowlist, not an executable-dir search.** `BUNDLED_PLUGINS` names exactly the three first-party plugins. Probing the executable's directory for *any* bare name would have quietly undone PR 3: on a `.deb` install `current_exe()` sits in `/usr/bin`, so "look next to the binary" is a slice of `PATH` wearing a different hat. `the_executable_dir_is_not_a_general_path_lookup` pins that — `sh`, `curl`, `python3` and even `dofek-tui` are all refused from the bundled dir.
> - Managed installs win over the bundled copy (the user's install was deliberate and may be newer). `$APPDIR/usr/bin` is probed for AppImage. `.exe` is tolerated either way in a Windows config.
> - **Symlink containment was generalised, not duplicated.** The escape check now lives in one `resolve_inside` helper applied to every candidate directory, and it returns `Err` (not "try the next dir") for a planted link, so an escape is reported rather than masked. This did change the error text — the existing test asserted the old wording, and now asserts the property plus the offending directory.
> - **The bundled probe accepts both `<name>` and `<name>-<triple>`.** Tauri's published sidecar docs state the *source* naming requirement but say nothing about whether the suffix survives into the installed bundle, and the verification build could not be completed (see the blocker below). Rather than encode a guess about a dependency's internals, both resolve; `build.rs` emits `DOFEK_TARGET_TRIPLE` from Cargo's `TARGET` since `std` exposes no equivalent. Two tests cover it, plain-name-wins included.
>
> **Done — channel-aware update hints (not gated on signing):**
>
> - `UpdateInfo` gained `channel: InstallChannel` and `hint: String`. Six channels detected from `current_exe()`: Homebrew (all three of `/opt/homebrew`, `/usr/local/Homebrew`, and the `Cellar`/`Caskroom` paths symlinks point into), WinGet, SystemPackage, AppImage (via `$APPDIR`), Development, Standalone.
> - **Ordering matters and is pinned by a test:** Intel Homebrew lives under `/usr/local`, which also matches the system-prefix rule, so Homebrew is checked first — and Development is checked before everything, since a build tree can sit under any prefix.
> - `InstallChannel::supports_in_app_update()` returns true only for Standalone and AppImage. This is the gate the updater wiring below will hang off: writing over dpkg- or brew-owned files leaves those databases lying about what is installed. `only_self_contained_channels_allow_in_app_update` asserts every package-managed channel is excluded.
> - **The hint is rendered in both UIs**, not just computed. A hint nobody displays is precisely the `plugin_label` mistake PR 5 had to go back and fix. TUI: a `WATCH_COLOR` line in the update overlay. GUI: a new `#update-hint` row written via `textContent`.
>
> **Deliberately not done — the updater itself.** `tauri-plugin-updater`, `latest.json`, and `createUpdaterArtifacts` are all still open, per sequencing constraint 3: shipping unsigned MSIs through an updater re-triggers SmartScreen on every update, which is worse than no updater. The channel gate above is the prerequisite that is now in place.
>
> **Also not done — one-click *registration* of a bundled plugin.** The binaries now ship and resolve, but a user still has to point the GUI file picker (or the CLI) at one to get a `plugins.toml` entry. Making that one click is a discrete UI/CLI unit — and it interacts with PR 3's `pending_plugin_paths` provenance gate — so it is called out rather than half-built. The infrastructure it needs is done: a `plugins.toml` entry naming a bare `dofek-ollama` already resolves with no copy.

- Wire `tauri-plugin-updater` + `tauri-plugin-process`; `createUpdaterArtifacts: true`; pubkey in `tauri.conf.json`; private key in Actions secrets **and a password manager — losing it permanently strands every installed copy**.
- `release.yml` gains a step emitting `latest.json` (version, pub_date, per-platform url + signature). The draft-release flow still gates it: `releases/latest/download/latest.json` only resolves after you click Publish.
- **Be explicit about who this serves**: MSI, AppImage, and dmg/.app only. Not deb/rpm, Homebrew, winget, or the standalone TUI. Keep `src/update.rs` as the shared "is there an update" brain; add channel-aware hints (detect `/opt/homebrew/` or `WinGet\Packages` in `current_exe()` → suggest `brew upgrade` / `winget upgrade`). Never silent auto-install.
- **Bundle the 3 plugins** via `externalBin` (they inherit signing/notarization for free), extend `prep-sidecar.{sh,ps1}` to stage them, and add a bundled-dir probe to `resolve_command` (`current_exe()` dir, plus `$APPDIR/usr/bin` for AppImage). Install becomes one click, offline.

---

## v1.8 — Integration surface *(the "AI-aware" story made real)*

`docs/tabby-integration.md` already specifies most of this and its design holds up, with corrections: don't serialize `Instant` (add a separate `timestamp_ms: u64` from `SystemTime`); no `rand` dep needed (`uuid` v4 is already a dependency); the token must be **mandatory**, not optional; and MCP should ship *before* `serve`, since the doc predates MCP being in scope.

**`src/api/mod.rs` (new)** — shared shaping layer under all three surfaces: `capture_snapshot()` (spawns collector, **discards the first tick** — sysinfo CPU% needs a delta and plugins send their manifest on poll #1), `SnapshotSummary`, `filter_processes()`, `ai_workload_view()`, and `kill_pid` moved here from `gui/src/lib.rs:295-313` so there's one implementation.

1. **`dofek-tui snapshot [--pretty] [--watch] [--count N]`** — zero new deps, ~1 day. NDJSON for `--watch`; treat `BrokenPipe` as clean exit 0 so `| head -3` and `| jq` behave; logging stays on stderr.
2. **`dofek-tui mcp`** — **hand-rolled sync JSON-RPC over stdio, zero new deps.** The official `rmcp` SDK is tokio-based, which would either break the no-tokio invariant or force a separate binary (breaking single-binary distribution and every installer script). MCP's stdio transport is newline-delimited JSON-RPC 2.0 — the exact framing this codebase already implements for plugins, just with dofek on the server side. ~400 lines: `initialize`, `notifications/initialized`, `tools/list`, `tools/call`, `ping`. Escape hatch documented in comments if protocol churn ever justifies `rmcp`.
   Tools (all read-only in v1): `get_system_snapshot` (no process list, token economy), **`get_ai_workloads`** (the differentiator — AI-classified processes with `ai_state`, VRAM, plugin labels like Ollama model names), `get_gpu_status`, `list_processes`, `find_process`. Results carry `age_ms`. Refresh floors at 2000ms.
   `kill_process` is **not listed by default** — behind `--allow-kill`, requiring *both* `pid` and `name` with a live re-verify (mitigates the PID-reuse race between an agent's observation and its action), SIGTERM only, refusing PID 0/1/self/parent, annotated `destructiveHint: true`.
3. **`dofek-tui serve`** — `tiny_http` (sync, ~6 small transitive crates). Rejected: hand-rolled `TcpListener` (HTTP parsing is exactly where this project already got burned twice), and tokio+axum (~150 crates for three GET endpoints). SSE fan-out solves the single-consumer mpsc problem: serve's main thread consumes the collector, serializes **once** per tick into an `Arc<str>`, and broadcasts to per-client bounded `sync_channel(4)`s — slow clients drop frames rather than stalling the tick.
   Security: hardcoded `127.0.0.1` (**no `--host` flag exists at all** — can't misconfigure what isn't there), mandatory bearer token auto-generated as a UUIDv4, constant-time compare, GET-only (anything else ⇒ 405), no CORS headers, worker pool of 4 with 32-request/8-SSE caps. Discovery file at `<config_dir>/dofek/serve.json` written `0o600`.

---

## v1.9 — Platform parity

### macOS Apple Silicon sensors (the biggest functional gap; 1–2 weeks of careful FFI)
Reference implementation: **macmon** (vladkens/macmon, MIT, Rust, maintained). Recommend hand-rolled FFI modeled on it (attribute in a header comment) using `core-foundation` + raw `extern "C"`, ~400–600 lines of unsafe — rather than depending on macmon as a library (it's binary-first with no lib API stability promise).

`src/data/macos/{mod,smc,ioreport,gpu}.rs`, mirroring the Linux block in the collector. Every source follows the **`RaplTracker` contract**: init once, self-disable on first failure, single `log::info` — mandatory, because CI macOS runners are VMs where these reads may fail under `-D warnings`.

- GPU util + memory: IORegistry `AGXAccelerator` → `PerformanceStatistics`. **Straightforward — ship this first even if the rest slips.**
- CPU/GPU temperature: AppleSMC user client, enumerate keys and filter `Tp*`/`Tg*` (names vary M1→M4 — never hardcode). Moderate.
- CPU/GPU power: IOReport "Energy Model" mJ counters, delta/time — exactly the RAPL model. Moderate; private framework, but well-trodden.
- **Unified memory honesty:** report `vram_used` = GPU-allocated system memory, `vram_total` = `recommendedMaxWorkingSetSize`, labeled "(unified)" via the existing `gpu_empty_state()` plumbing. **Per-process GPU memory is not exposed by public APIs — do not promise it.** `ai_detect` already degrades to name-based classification when `vram_bytes` is `None`.
- Move `nvml-wrapper` to `[target.'cfg(any(windows, target_os = "linux"))'.dependencies]` — dead weight on macOS.

### AMD GPU (Linux — highest value-per-line in the whole plan)
`src/data/amdgpu.rs`, `cfg(linux)`, pure `std::fs`, zero new deps, same style as `rapl.rs`. Enumerate `/sys/class/drm/card*/device/` where `uevent` contains `DRIVER=amdgpu`; read `gpu_busy_percent`, `mem_info_vram_{used,total}`, `hwmon/*/temp1_input`, `power1_average`. **Append** to NVML devices rather than replacing (mixed rigs exist).

**Per-process VRAM on AMD is genuinely hard** — DRM fdinfo requires scanning every fd of every process and can't read other users' processes without root. Defer behind a config flag or skip and document. Windows AMD without LHM (PDH `GPU Engine`/`GPU Process Memory` counters) is its own future release — finicky instance-name parsing; keep LHM as the documented path for now.

### Also fold in here (small, deferred from the review)
Net sparkline NIC identity (`app.rs:257` — pin to a stable interface instead of `first()` of a traffic-sorted list); gate the LHM probe to `cfg(windows)` (removes a ~2s first-snapshot stall on Linux/macOS); shrink `gpu_util_per_device` when a GPU disappears; remove the blanket `#![allow(dead_code)]` from `main.rs:1` and clean what it reveals.

---

# Verification

**Per PR:** `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` on all 3 OSes (CI already enforces this; the `--all-features` addition is new for the mock-plugin gate).

**Security fixes — reproduce first, then fix:**
- Finding 1: rename a process to `<img src=x onerror=alert(1)>`, confirm it fires in the GUI, then confirm it renders as inert text.
- Finding 2: drop a hostile `dofek.toml` with a `[[plugins]]` entry into a scratch dir, `cd` there, run `dofek-tui`, confirm execution — then confirm it's ignored.
- Finding 3: run a process named with CJK/emoji at the truncation boundary, confirm the TUI panics, then confirm it renders.
- Finding 4: `dofek-mock-plugin --hang`, confirm all metrics freeze, then confirm `tick()` stays <50ms.

**Integration surface:** `dofek-tui snapshot | jq .cpu.total_load` returns nonzero; `--watch | head -3` exits 0; `claude mcp add dofek -- dofek-tui mcp` then ask "what's using my GPU"; serve gets the curl/SSE/auth checklist from `docs/tabby-integration.md:116-124` plus 401-without-token, 405-on-POST, 503-on-9th-SSE-client.

**Regression:** TUI and GUI both boot and render after the serde changes (additive); GUI kill still works after `kill_pid` moves to `src/api/`.

**Release:** use the existing `dofek-version-bump` skill to keep the 5 version surfaces in sync.

---

# Cost summary
- **$0** — SignPath Foundation (Windows signing, OSS program)
- **$99/yr** — Apple Developer Program (macOS signing + notarization), unavoidable
- **~$120/yr** — Azure Trusted Signing, only if SignPath falls through *(verify country eligibility before relying on it)*
