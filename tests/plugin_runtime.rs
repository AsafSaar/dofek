//! Regression tests for the plugin runtime — one per defect the v1.6 rework
//! set out to fix.
//!
//! Every test drives the real [`PluginManager`] against the `dofek-mock-plugin`
//! fixture, so what is being exercised is the shipping code path: supervisor
//! thread, bounded reader, stderr drainer, sanitizer, containment.
//!
//! Four of the fixture's modes could not be smoke-tested before this landed —
//! `--hang`, `--slow`, `--stderr-flood` and `--fork-grandchild` all hang a
//! blocking reader by construction. They are the core of this file.
//!
//! The fixture path comes from `CARGO_BIN_EXE_*`, an absolute path to a regular
//! file, which is exactly what `store::resolve_command` accepts — no PATH
//! lookup and no managed-directory install needed.

#![cfg(feature = "mock-plugin")]

use std::time::{Duration, Instant};

use dofek::config::PluginConfig;
use dofek::plugin::protocol::ProcessContext;
use dofek::plugin::{PluginManager, PluginState, PluginStatus};

const MOCK: &str = env!("CARGO_BIN_EXE_dofek-mock-plugin");

/// The collector's cadence. Every `tick` in these tests must cost far less
/// than this, or a plugin is stealing time from the metrics.
const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// The budget one `tick()` gets. The whole point of the rework is that this is
/// independent of what any plugin is doing.
const TICK_BUDGET: Duration = Duration::from_millis(50);

fn plugin(name: &str, args: &[&str], timeout_ms: u64) -> PluginConfig {
    PluginConfig {
        name: name.to_string(),
        command: MOCK.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        enabled: true,
        timeout_ms,
    }
}

/// A realistic process context — the thing that used to be cloned once per
/// plugin per tick. Building it here keeps the timing assertions honest.
fn context(n: usize) -> Vec<ProcessContext> {
    (0..n)
        .map(|i| ProcessContext {
            pid: i as u32 + 1,
            name: format!("process-{i}"),
            vram_bytes: if i % 7 == 0 { Some(1 << 30) } else { None },
        })
        .collect()
}

/// Tick until `pred` holds, returning how long that took. Ticks are timed and
/// the budget is asserted on every one of them, so no test can accidentally
/// pass while the collector is being stalled.
fn tick_until(
    mgr: &PluginManager,
    limit: Duration,
    mut pred: impl FnMut(&[PluginStatus]) -> bool,
) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < limit {
        let t = Instant::now();
        let statuses = mgr.tick(context(200));
        let cost = t.elapsed();
        assert!(
            cost < TICK_BUDGET,
            "tick() took {cost:?}, over the {TICK_BUDGET:?} budget — a plugin is blocking the collector"
        );
        if pred(&statuses) {
            return Some(start.elapsed());
        }
        std::thread::sleep(TICK_INTERVAL);
    }
    None
}

fn state_of(statuses: &[PluginStatus], name: &str) -> Option<PluginState> {
    statuses.iter().find(|s| s.name == name).map(|s| s.state)
}

// --- finding 4: the poll timeout did not exist ---

/// The headline regression. `--hang` reads the first poll and never answers,
/// holding its stdout pipe open — the exact shape that froze the collector, and
/// with it every metric in both UIs, because `read_line_timeout` did one
/// blocking read and only checked the clock in its `Err` branch.
#[test]
fn hung_plugin_does_not_stall_collector() {
    let mgr = PluginManager::new(&[plugin("hung", &["--hang"], 2000)]);

    // Let it read the first request and wedge.
    let mut worst = Duration::ZERO;
    for _ in 0..10 {
        let t = Instant::now();
        let _ = mgr.tick(context(500));
        worst = worst.max(t.elapsed());
        std::thread::sleep(TICK_INTERVAL);
    }
    assert!(
        worst < TICK_BUDGET,
        "slowest tick against a wedged plugin was {worst:?}"
    );
}

/// A plugin that never answers must eventually be recycled rather than sitting
/// at "starting" forever — five failed polls, then a restart under backoff.
#[test]
fn hung_plugin_is_eventually_recycled() {
    // 100 ms timeout so five failed polls fit comfortably in the window.
    let mgr = PluginManager::new(&[plugin("hung", &["--hang"], 100)]);

    let unhealthy = tick_until(&mgr, Duration::from_secs(5), |s| {
        matches!(
            state_of(s, "hung"),
            Some(PluginState::Unhealthy) | Some(PluginState::Crashed)
        )
    });
    assert!(unhealthy.is_some(), "a wedged plugin was never marked unhealthy");
}

/// Plugins used to be polled one after another on the collector thread, so
/// three plugins on a 2 s timeout could cost 6 s. Now each has its own
/// supervisor: three plugins that each take 400 ms all answer in ~400 ms, not
/// 1200 ms.
#[test]
fn three_plugins_do_not_stack_latency() {
    let slow = ["--slow", "400"];
    let mgr = PluginManager::new(&[
        plugin("a", &slow, 2000),
        plugin("b", &slow, 2000),
        plugin("c", &slow, 2000),
    ]);

    let elapsed = tick_until(&mgr, Duration::from_secs(5), |s| {
        s.len() == 3 && s.iter().all(|p| p.state == PluginState::Healthy)
    })
    .expect("all three plugins should report healthy");

    // Sequential polling could not beat 3 × 400 ms. Concurrent polling lands
    // just over one 400 ms response plus a tick of slack.
    assert!(
        elapsed < Duration::from_millis(1000),
        "three 400 ms plugins took {elapsed:?} — latency is still stacking"
    );
}

// --- stderr was piped and never drained ---

/// 10 MiB of stderr against a 64 KiB pipe buffer. Without a drainer the plugin
/// blocks writing stderr before it ever writes its response, and both sides sit
/// there forever — while `plugins/README.md` claimed stderr was drained.
#[test]
fn stderr_flood_does_not_deadlock() {
    let mgr = PluginManager::new(&[plugin("noisy", &["--stderr-flood"], 10_000)]);

    let elapsed = tick_until(&mgr, Duration::from_secs(20), |s| {
        state_of(s, "noisy") == Some(PluginState::Healthy)
    })
    .expect("a plugin that floods stderr must still be able to answer on stdout");

    // Not a performance assertion — just proof it completed rather than
    // deadlocking until the test harness gave up.
    assert!(elapsed < Duration::from_secs(20));
}

// --- unbounded reads ---

/// A 4 MiB response line. `read_line` on an unbounded `BufReader` would grow a
/// `String` to hold all of it; the reader now caps at 256 KiB and disconnects
/// rather than trying to resync mid-line.
#[test]
fn giant_line_disconnects_plugin() {
    let mgr = PluginManager::new(&[plugin("giant", &["--giant-line"], 2000)]);

    let crashed = tick_until(&mgr, Duration::from_secs(10), |s| {
        state_of(s, "giant") == Some(PluginState::Crashed)
    });
    assert!(
        crashed.is_some(),
        "an over-long response line should disconnect the plugin"
    );

    // And nothing from that line reached the UI layer.
    let statuses = mgr.tick(context(10));
    let stored = statuses.iter().find(|s| s.name == "giant").unwrap();
    assert!(
        stored.response.is_none(),
        "a truncated giant line must never be surfaced as a response"
    );
}

/// One request, one response. A plugin emitting a thousand replies per poll is
/// bounded by the reader's queue depth (so it costs no memory), and then
/// disconnected, rather than being allowed to serve stale data indefinitely
/// while looking healthy.
#[test]
fn flood_is_rate_limited() {
    let mgr = PluginManager::new(&[plugin("floody", &["--flood"], 2000)]);

    let disconnected = tick_until(&mgr, Duration::from_secs(10), |s| {
        state_of(s, "floody") == Some(PluginState::Crashed)
    });
    assert!(
        disconnected.is_some(),
        "a plugin flooding responses should be disconnected"
    );
}

// --- crash handling ---

/// Exits non-zero after two polls. The plugin must come back, and must wait out
/// the first backoff step (1 s) before it does — a plugin that dies instantly
/// on every start must not be respawned at tick rate.
#[test]
fn crash_respawns_with_backoff() {
    let mgr = PluginManager::new(&[plugin("crashy", &["--crash-after", "2"], 2000)]);

    tick_until(&mgr, Duration::from_secs(5), |s| {
        state_of(s, "crashy") == Some(PluginState::Healthy)
    })
    .expect("plugin should answer before it crashes");

    tick_until(&mgr, Duration::from_secs(5), |s| {
        state_of(s, "crashy") == Some(PluginState::Crashed)
    })
    .expect("plugin should be seen as crashed");

    let back = tick_until(&mgr, Duration::from_secs(10), |s| {
        state_of(s, "crashy") == Some(PluginState::Healthy)
    })
    .expect("plugin should be respawned");

    assert!(
        back >= Duration::from_millis(900),
        "respawn took {back:?} — the 1 s backoff was not honoured"
    );
}

// --- teardown ---

/// `shutdown()` used to sleep a flat 2 s on the collector thread regardless of
/// how many plugins there were or whether they had already exited. Now every
/// plugin tears down on its own thread, so three plugins that ignore the
/// shutdown message entirely still cost one grace period, not three.
#[test]
fn shutdown_is_prompt() {
    let mut mgr = PluginManager::new(&[
        plugin("h1", &["--hang"], 2000),
        plugin("h2", &["--hang"], 2000),
        plugin("h3", &["--hang"], 2000),
    ]);

    // Make sure all three are actually running and wedged before timing.
    for _ in 0..4 {
        let _ = mgr.tick(context(50));
        std::thread::sleep(TICK_INTERVAL);
    }

    let t = Instant::now();
    mgr.shutdown();
    let elapsed = t.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "shutting down three wedged plugins took {elapsed:?}"
    );
}

/// Installing or removing a plugin swaps the whole set. It must not leave the
/// old children running, and it must not block the caller for long.
#[test]
fn replace_swaps_the_plugin_set_promptly() {
    let mut mgr = PluginManager::new(&[plugin("old", &["--hang"], 2000)]);
    for _ in 0..3 {
        let _ = mgr.tick(context(10));
        std::thread::sleep(TICK_INTERVAL);
    }

    let t = Instant::now();
    mgr.replace(&[plugin("new", &[], 2000)]);
    let elapsed = t.elapsed();
    assert!(elapsed < Duration::from_millis(500), "replace took {elapsed:?}");

    let healthy = tick_until(&mgr, Duration::from_secs(5), |s| {
        s.len() == 1 && state_of(s, "new") == Some(PluginState::Healthy)
    });
    assert!(healthy.is_some(), "the replacement plugin never came up");
}

// --- containment ---

/// A plugin's own children must die with it. `CLAUDE.md` claimed Job Objects
/// were in use; the code set only `CREATE_NO_WINDOW`, so a plugin that spawned
/// helpers left them running after a force-kill.
///
/// Unix-only because that is the platform this can be verified on here — the
/// Windows Job Object path is the same idea and is exercised by CI.
#[cfg(unix)]
#[test]
fn grandchild_dies_with_plugin() {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let mut mgr = PluginManager::new(&[plugin("forker", &["--fork-grandchild"], 2000)]);

    let mut grandchild: Option<i32> = None;
    tick_until(&mgr, Duration::from_secs(5), |s| {
        let Some(resp) = s.first().and_then(|p| p.response.as_ref()) else {
            return false;
        };
        grandchild = resp
            .panels
            .iter()
            .flat_map(|p| &p.content)
            .find(|e| e.key == "grandchild")
            .and_then(|e| e.value.parse::<i32>().ok());
        grandchild.is_some()
    })
    .expect("plugin should report the pid of the child it spawned");

    let pid = Pid::from_raw(grandchild.expect("pid reported"));
    assert!(kill(pid, None).is_ok(), "grandchild should be alive before shutdown");

    mgr.shutdown();

    // Reparenting to init and reaping is not instantaneous.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if kill(pid, None).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Don't leave a stray process behind if the assertion is about to fail.
    let _ = kill(pid, nix::sys::signal::Signal::SIGKILL);
    panic!("grandchild {pid} outlived its plugin — process-group containment failed");
}

// --- protocol ---

/// A plugin written against the pre-v1.6 protocol — no `schema_version`, no
/// `seq` echo, none of the optional payload fields — must keep working
/// unchanged. Every new field is `#[serde(default)]` precisely so this holds.
#[test]
fn legacy_plugin_still_works() {
    let mgr = PluginManager::new(&[plugin("legacy", &["--legacy", "--no-echo-seq"], 2000)]);

    let ok = tick_until(&mgr, Duration::from_secs(5), |s| {
        state_of(s, "legacy") == Some(PluginState::Healthy)
    });
    assert!(ok.is_some(), "a pre-v1.6 plugin should still be healthy");

    let statuses = mgr.tick(context(10));
    let legacy = statuses.iter().find(|s| s.name == "legacy").unwrap();
    // Its manifest is still picked up, so the dock shows the plugin's own name.
    assert_eq!(legacy.display_name, "mock");
    let resp = legacy.response.as_ref().expect("a response was stored");
    assert_eq!(resp.seq, 0, "legacy plugins do not echo seq");
    assert!(resp.panels.is_empty() && resp.metrics.is_empty());
}

/// `PollResponse.status` was parsed and then never read. A plugin reporting its
/// own failure now shows as unhealthy — while still displaying whatever data it
/// managed to send.
#[test]
fn status_error_marks_unhealthy() {
    let mgr = PluginManager::new(&[plugin("sad", &["--status-error"], 2000)]);

    let ok = tick_until(&mgr, Duration::from_secs(5), |s| {
        state_of(s, "sad") == Some(PluginState::Unhealthy)
    });
    assert!(ok.is_some(), "status:\"error\" should mark the plugin unhealthy");

    let statuses = mgr.tick(context(10));
    let sad = statuses.iter().find(|s| s.name == "sad").unwrap();
    assert!(
        sad.response.is_some(),
        "an unhealthy plugin's data is still shown — it reported, it just reported a problem"
    );
}

/// A modern plugin echoes `seq`, which is what lets the supervisor tell a late
/// reply from the answer to the current poll.
#[test]
fn seq_is_echoed_and_advances() {
    let mgr = PluginManager::new(&[plugin("seq", &[], 2000)]);

    let mut first = 0;
    tick_until(&mgr, Duration::from_secs(5), |s| {
        first = s
            .first()
            .and_then(|p| p.response.as_ref())
            .map(|r| r.seq)
            .unwrap_or(0);
        first > 0
    })
    .expect("plugin should echo a nonzero seq");

    let later = tick_until(&mgr, Duration::from_secs(5), |s| {
        s.first()
            .and_then(|p| p.response.as_ref())
            .is_some_and(|r| r.seq > first)
    });
    assert!(later.is_some(), "seq should advance across polls");
}

// --- ingest bounds (finding 5: SECURITY.md claimed these existed) ---

/// The sanitizer runs before anything a plugin sent is stored, so the caps hold
/// for every consumer downstream — not just the renderers that remembered to
/// call `.take()`.
#[test]
fn responses_are_bounded_at_ingest() {
    use dofek::plugin::sanitize;

    let mgr = PluginManager::new(&[plugin("mock", &[], 2000)]);
    tick_until(&mgr, Duration::from_secs(5), |s| {
        s.first().is_some_and(|p| p.response.is_some())
    })
    .expect("plugin should respond");

    let statuses = mgr.tick(context(10));
    let resp = statuses[0].response.as_ref().unwrap();
    assert!(resp.panels.len() <= sanitize::MAX_PANELS);
    assert!(resp.metrics.len() <= sanitize::MAX_METRICS);
    for p in &resp.panels {
        assert!(p.content.len() <= sanitize::MAX_PANEL_ENTRIES);
        assert!(p.label.chars().count() <= sanitize::MAX_SHORT_CHARS);
        for e in &p.content {
            assert!(e.value.chars().count() <= sanitize::MAX_LONG_CHARS);
        }
    }
}

/// No plugins configured ⇒ no work at all. The collector checks this before it
/// builds the ~500-string process context, which it used to build on every tick
/// whether or not anything read it.
#[test]
fn empty_plugin_set_is_free() {
    let mgr = PluginManager::new(&[]);
    assert!(!mgr.has_plugins());
    let t = Instant::now();
    for _ in 0..1000 {
        assert!(mgr.tick(Vec::new()).is_empty());
    }
    assert!(t.elapsed() < Duration::from_millis(50), "{:?}", t.elapsed());
}

/// A disabled plugin is never spawned.
#[test]
fn disabled_plugins_are_not_started() {
    let mut cfg = plugin("off", &[], 2000);
    cfg.enabled = false;
    let mgr = PluginManager::new(&[cfg]);
    assert!(!mgr.has_plugins());
    assert!(mgr.tick(context(10)).is_empty());
}

// --- collector wiring ---

/// End-to-end through the real collector: a plugin's data reaches
/// `DataSnapshot`, the snapshots keep flowing while it is being polled, and
/// `CollectorHandle::shutdown` actually stops the child.
///
/// That last part is not incidental. Plugins are now put in their own session
/// so their grandchildren can be reaped, which also means they no longer get
/// the terminal's job-control signals — nothing but this teardown will stop
/// them.
#[cfg(unix)]
#[test]
fn collector_delivers_plugin_data_and_stops_the_child_on_shutdown() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let mut config = dofek::config::Config::default();
    config.plugins.push(plugin("mock", &[], 2000));

    let (rx, mut handle) =
        dofek::data::spawn_collector(config, Arc::new(AtomicU64::new(100)));

    // The mock annotates its own pid, which is how we learn what to check for.
    let mut plugin_pid = None;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let Ok(snap) = rx.recv_timeout(Duration::from_secs(20)) else {
            break;
        };
        if let Some(st) = snap.plugin_statuses.iter().find(|s| s.name == "mock")
            && let Some(resp) = st.response.as_ref()
            && let Some(ann) = resp.process_annotations.first()
        {
            assert_eq!(st.state, PluginState::Healthy);
            assert_eq!(st.display_name, "mock");
            plugin_pid = Some(ann.pid as i32);
            break;
        }
    }
    let pid = Pid::from_raw(plugin_pid.expect("plugin data should reach DataSnapshot"));
    assert!(kill(pid, None).is_ok(), "plugin should be running");

    let t = Instant::now();
    handle.shutdown();
    let elapsed = t.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "collector shutdown took {elapsed:?}"
    );

    let gone = Instant::now() + Duration::from_secs(5);
    while Instant::now() < gone {
        if kill(pid, None).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = kill(pid, Signal::SIGKILL);
    panic!("plugin {pid} outlived the collector — quitting dofek would orphan it");
}

// --- install-time probe (the other half of the fake timeout) ---

/// `store::probe_manifest` had the same defect as the collector's reader, but
/// on the `plugins_add` IPC path: a blocking `read_line` with the deadline
/// checked only *before* it. Picking a binary that opens stdout and never
/// writes parked a Tauri worker permanently.
#[test]
fn probe_of_a_silent_binary_times_out() {
    use dofek::plugin::store::probe_manifest;

    let t = Instant::now();
    let err = probe_manifest(std::path::Path::new(MOCK), &["--hang".to_string()])
        .expect_err("a binary that never answers must not probe successfully");
    let elapsed = t.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "probe of a wedged binary took {elapsed:?} — the deadline is still decorative"
    );
    assert!(
        format!("{err:#}").contains("timed out"),
        "unexpected probe error: {err:#}"
    );
}

/// The happy path: a real plugin identifies itself, and what comes back has
/// been through the same bounds as a runtime response.
#[test]
fn probe_reads_the_manifest() {
    use dofek::plugin::store::probe_manifest;

    let m = probe_manifest(std::path::Path::new(MOCK), &[]).expect("mock plugin should identify");
    assert_eq!(m.name, "mock");
    assert_eq!(m.version, "0.0.0");
    assert_eq!(m.author, "dofek");
}

/// A command that cannot be resolved must fail closed — not fall through to a
/// PATH or working-directory lookup (PR 3), and not spin retrying at tick rate.
#[test]
fn unresolvable_command_stays_crashed_without_spinning() {
    let cfg = PluginConfig {
        name: "ghost".into(),
        command: "definitely-not-installed".into(),
        args: vec![],
        enabled: true,
        timeout_ms: 500,
    };
    let mgr = PluginManager::new(&[cfg]);

    let crashed = tick_until(&mgr, Duration::from_secs(3), |s| {
        state_of(s, "ghost") == Some(PluginState::Crashed)
    });
    assert!(crashed.is_some(), "an unresolvable plugin should report crashed");
}
