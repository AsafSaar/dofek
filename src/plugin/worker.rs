//! One supervisor thread per plugin.
//!
//! ## Why a thread per plugin rather than a timeout on the collector
//!
//! The obvious minimal fix for the fake `timeout_ms` is a reader thread plus
//! `recv_timeout` on the collector thread. It closes the freeze, but it leaves
//! three things behind:
//!
//! * **Latency still stacks.** Plugins are polled one after another, so three
//!   plugins on a 2 s timeout can cost 6 s of collector time in the worst case
//!   — and the collector is what produces *every* metric in both UIs.
//! * **Teardown still blocks.** `shutdown()`'s 2 s sleep and `replace()`'s
//!   200 ms sleep run on the collector, so quitting or installing a plugin
//!   stalls the whole monitor.
//! * **Spawn still blocks.** `PluginManager::new` spawned every child inline.
//!
//! Giving each plugin its own supervisor makes all of those per-plugin and
//! concurrent. Plugin counts are single-digit and this codebase is already
//! thread-and-channel shaped, so three threads per plugin (supervisor, stdout
//! reader, stderr drainer) is a proportionate cost.
//!
//! ## What the supervisor guarantees
//!
//! * [`PluginManager::tick`](super::PluginManager::tick) never blocks on a
//!   plugin: it swaps the shared process context, nudges each supervisor, and
//!   reads the last-known status. A wedged plugin costs the collector nothing.
//! * Every wait inside the supervisor is bounded, and the loop re-checks its
//!   stop flag on a fixed slice, so joining it is bounded too.
//! * Responses are sanitized at ingest — see [`super::sanitize`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::process::{PluginProcess, ReadEvent};
use super::protocol::{PluginManifest, PollResponse, ProcessContext};
use super::sanitize;
use super::{PluginState, PluginStatus};
use crate::config::PluginConfig;

/// How often the supervisor re-checks its stop flag while otherwise idle.
/// Bounds how long `shutdown()` waits before a supervisor notices.
const IDLE_SLICE: Duration = Duration::from_millis(50);

/// Consecutive failed polls before the plugin is shown as unhealthy.
const UNHEALTHY_AFTER: u32 = 3;

/// Consecutive failed polls before the child is killed and restarted under
/// backoff. Above [`UNHEALTHY_AFTER`] so a plugin that is merely slow gets a
/// visible warning before it gets recycled.
const RESTART_AFTER: u32 = 5;

/// Extra unsolicited lines in one drain that count as a flood.
///
/// The protocol is strictly one response per request, so a stray extra line is
/// tolerated (a plugin answering a request we had already given up on) but a
/// steady stream is not. The ceiling is [`process::RESPONSE_QUEUE`]: the reader
/// blocks once its queue is full, so no single drain can ever see more than
/// that. This threshold sits just under it — "the queue was essentially full of
/// lines nobody asked for".
const FLOOD_LINES: u32 = 3;

/// Consecutive flooding polls before the plugin is disconnected.
///
/// The queue depth already bounds the *memory* a flooding plugin can cost. What
/// this stops is the desync: a plugin emitting a thousand lines per request
/// would otherwise spend hundreds of ticks serving us stale data that looks
/// perfectly healthy. Disconnect-and-backoff surfaces it instead.
const FLOOD_BEFORE_DISCONNECT: u32 = 3;

/// Poll request serialized straight out of the shared context.
///
/// A borrowing mirror of [`super::protocol::PollRequest`], which owns its
/// `Vec<ProcessContext>`. The owned form meant every plugin got its own
/// `to_vec()` of the process list — ~500 `String` clones per plugin per tick,
/// paid whether or not any plugin cared. Serializing by reference off one
/// shared `Arc` removes that entirely. The field set and order must stay in
/// step with `PollRequest`; `request_wire_format_matches_the_owned_type`
/// pins it.
#[derive(Serialize)]
struct PollRequestRef<'a> {
    #[serde(rename = "type")]
    msg_type: &'static str,
    schema_version: u32,
    seq: u64,
    timestamp_ms: u64,
    processes: &'a [ProcessContext],
}

/// The process list handed to every plugin on a tick. Swapped wholesale by
/// [`PluginManager::tick`](super::PluginManager::tick); supervisors clone the
/// `Arc`, never the contents.
pub type SharedContext = Arc<Mutex<Arc<Vec<ProcessContext>>>>;

/// Handle to one plugin's supervisor thread.
pub struct PluginWorker {
    status: Arc<Mutex<PluginStatus>>,
    stop: Arc<AtomicBool>,
    /// Dropped by `stop_signal()` so a supervisor parked in `recv_timeout`
    /// wakes immediately instead of waiting out its slice.
    wake: Option<SyncSender<()>>,
    join: Option<JoinHandle<()>>,
}

impl PluginWorker {
    /// Start a supervisor for `config`. Returns as soon as the thread is
    /// running — the child is spawned on that thread, so a slow or failing
    /// spawn never delays the caller.
    pub fn start(config: PluginConfig, ctx: SharedContext) -> Self {
        let status = Arc::new(Mutex::new(PluginStatus {
            name: config.name.clone(),
            display_name: config.name.clone(),
            state: PluginState::Starting,
            response: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        // Depth 1: a tick that arrives while the supervisor is mid-poll is
        // dropped rather than queued. Queueing them would let a slow plugin
        // accumulate a backlog of requests it can never catch up on.
        let (wake, wake_rx) = sync_channel(1);

        let join = thread::Builder::new()
            .name(format!("plugin-sup:{}", config.name))
            .spawn({
                let status = Arc::clone(&status);
                let stop = Arc::clone(&stop);
                move || Supervisor::new(config, status, ctx).run(&stop, wake_rx)
            })
            .ok();

        Self {
            status,
            stop,
            wake: Some(wake),
            join,
        }
    }

    /// Ask the supervisor to run one poll. Non-blocking: a supervisor that is
    /// still working on the previous tick simply misses this one.
    pub fn nudge(&self) {
        if let Some(w) = self.wake.as_ref() {
            let _ = w.try_send(());
        }
    }

    /// Snapshot of what this plugin last reported.
    pub fn status(&self) -> PluginStatus {
        match self.status.lock() {
            Ok(s) => s.clone(),
            // A supervisor that panicked mid-update leaves the mutex poisoned.
            // The data is still structurally valid, and reporting the plugin
            // as crashed is more useful than propagating the panic into the
            // collector.
            Err(p) => p.into_inner().clone(),
        }
    }

    /// Tell the supervisor to wind down, without waiting for it. Split from
    /// [`PluginWorker::join`] so a manager can signal every plugin first and
    /// then join them — making teardown concurrent rather than sequential.
    pub fn stop_signal(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake = None; // wakes a supervisor parked in recv_timeout
    }

    /// Wait for the supervisor to finish. Bounded by construction: every wait
    /// inside the loop is time-limited and the stop flag is checked each slice.
    pub fn join(&mut self) {
        if let Some(h) = self.join.take()
            && h.join().is_err()
        {
            log::warn!("plugin supervisor thread panicked during shutdown");
        }
    }
}

impl Drop for PluginWorker {
    fn drop(&mut self) {
        self.stop_signal();
        self.join();
    }
}

/// What one request/response exchange produced.
enum Outcome {
    /// A parsed, sanitized response for the request we just sent.
    Response(Box<PollResponse>),
    /// No reply within `timeout_ms`.
    Timeout,
    /// The plugin answered, but not with something we can use. It is still
    /// alive, so this is a soft error.
    Protocol(String),
    /// The child, or its stdout, is gone.
    Dead(String),
    /// We were told to shut down mid-poll. Not the plugin's fault, so it must
    /// not be counted against its health.
    Stopped,
}

/// Per-plugin state owned entirely by its supervisor thread.
struct Supervisor {
    config: PluginConfig,
    status: Arc<Mutex<PluginStatus>>,
    ctx: SharedContext,
    proc: Option<PluginProcess>,
    manifest: Option<PluginManifest>,
    seq: u64,
    consecutive_errors: u32,
    consecutive_floods: u32,
    crash_count: u32,
    last_crash: Option<Instant>,
}

impl Supervisor {
    fn new(config: PluginConfig, status: Arc<Mutex<PluginStatus>>, ctx: SharedContext) -> Self {
        Self {
            config,
            status,
            ctx,
            proc: None,
            manifest: None,
            seq: 0,
            consecutive_errors: 0,
            consecutive_floods: 0,
            crash_count: 0,
            last_crash: None,
        }
    }

    fn run(mut self, stop: &AtomicBool, wake: Receiver<()>) {
        self.spawn_child();
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            match wake.recv_timeout(IDLE_SLICE) {
                Ok(()) => self.poll_once(stop),
                // No tick this slice — loop back and re-check the stop flag.
                Err(RecvTimeoutError::Timeout) => {}
                // The manager dropped its sender: shutdown.
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        if let Some(mut p) = self.proc.take() {
            p.shutdown_and_kill();
        }
    }

    // --- spawn / backoff ---

    fn backoff(&self) -> Duration {
        Duration::from_secs(match self.crash_count {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 16,
            _ => 30,
        })
    }

    fn may_respawn(&self) -> bool {
        self.last_crash.is_none_or(|t| t.elapsed() >= self.backoff())
    }

    fn spawn_child(&mut self) {
        match PluginProcess::spawn(&self.config.command, &self.config.args, &self.config.name) {
            Ok(p) => {
                log::info!(
                    "plugin '{}' spawned (command: {}, pid {})",
                    self.config.name,
                    self.config.command,
                    p.pid()
                );
                self.proc = Some(p);
                self.consecutive_errors = 0;
                self.consecutive_floods = 0;
                self.set_state(PluginState::Starting);
            }
            Err(e) => {
                log::error!("failed to spawn plugin '{}': {e:#}", self.config.name);
                self.on_death("spawn failed");
            }
        }
    }

    /// Record a dead child and schedule the next attempt. Backoff is driven by
    /// `crash_count`, so a plugin that dies instantly on every start settles at
    /// one attempt per 30 s rather than spinning.
    fn on_death(&mut self, why: &str) {
        if let Some(mut p) = self.proc.take() {
            p.kill();
        }
        self.crash_count += 1;
        self.last_crash = Some(Instant::now());
        self.consecutive_errors = 0;
        self.consecutive_floods = 0;
        log::warn!(
            "plugin '{}' disconnected: {why} (restart in {:?})",
            self.config.name,
            self.backoff()
        );
        self.set_state(PluginState::Crashed);
    }

    // --- one poll cycle ---

    fn poll_once(&mut self, stop: &AtomicBool) {
        if self.proc.is_none() {
            if !self.may_respawn() {
                return;
            }
            log::info!(
                "respawning plugin '{}' (attempt {})",
                self.config.name,
                self.crash_count + 1
            );
            self.spawn_child();
            if self.proc.is_none() {
                return;
            }
        }

        // A child that has exited shows up as `Closed` from its reader thread —
        // either here, in the drain, or in the wait below. There is deliberately
        // no `try_wait` liveness probe: `try_wait` reaps, and a reaped pid is a
        // process group id we no longer own. See `PluginProcess::reaped`.
        //
        // Anything still queued belongs to an earlier request — either a reply
        // that arrived after we gave up on it, or an extra line from a plugin
        // that answers more than once. Clearing it here is what stops a late
        // reply from being read as the answer to *this* request.
        match Self::drain_stale(self.proc.as_mut().expect("checked above")) {
            Ok(0) => self.consecutive_floods = 0,
            Ok(n) => {
                log::debug!(
                    "plugin '{}': discarded {n} unsolicited response line(s)",
                    self.config.name
                );
                if n >= FLOOD_LINES {
                    self.consecutive_floods += 1;
                    if self.consecutive_floods >= FLOOD_BEFORE_DISCONNECT {
                        self.on_death("plugin floods responses (more than one per request)");
                        return;
                    }
                } else {
                    self.consecutive_floods = 0;
                }
            }
            Err(why) => {
                self.on_death(&why);
                return;
            }
        }

        self.seq += 1;
        let seq = self.seq;
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Clone the Arc, not the processes, and release the lock before any
        // I/O — the collector must never wait on a plugin to take this lock.
        let processes = match self.ctx.lock() {
            Ok(g) => Arc::clone(&g),
            Err(p) => Arc::clone(&p.into_inner()),
        };

        let json = match serde_json::to_string(&PollRequestRef {
            msg_type: "poll",
            schema_version: super::protocol::SCHEMA_VERSION,
            seq,
            timestamp_ms,
            processes: &processes,
        }) {
            Ok(j) => j,
            Err(e) => {
                // Our own data failed to serialize — a bug on our side, not
                // the plugin's. Don't punish the plugin for it.
                log::error!("failed to serialize poll request: {e}");
                return;
            }
        };
        drop(processes);

        let timeout = Duration::from_millis(self.config.timeout_ms.max(1));
        let proc = self.proc.as_mut().expect("checked above");
        let outcome = match proc.send_line(&json) {
            Ok(()) => Self::await_response(proc, seq, timeout, stop),
            Err(e) => Outcome::Dead(format!("write to plugin stdin failed: {e}")),
        };
        self.apply(outcome);
    }

    /// Consume everything already queued. Returns how many lines were thrown
    /// away, or the reason the connection is finished.
    fn drain_stale(proc: &mut PluginProcess) -> Result<u32, String> {
        let mut n = 0;
        loop {
            match proc.try_recv() {
                Ok(ReadEvent::Line(_)) => n += 1,
                Ok(ReadEvent::Closed(why)) => return Err(why),
                Err(TryRecvError::Empty) => return Ok(n),
                Err(TryRecvError::Disconnected) => return Err("plugin reader stopped".into()),
            }
        }
    }

    /// Wait up to `timeout` for the reply to request `seq`.
    ///
    /// A plugin that echoes `seq` lets us skip a stale reply and keep waiting
    /// for the real one; a plugin that doesn't (`seq == 0`, which is every
    /// plugin written before v1.6) gets first-reply-wins, which combined with
    /// `drain_stale` is the best that can be done without an echo.
    ///
    /// The wait is sliced rather than done in one `recv_timeout` so that
    /// `stop` is honoured promptly: otherwise quitting dofek while a plugin is
    /// mid-poll would cost that plugin's full `timeout_ms` — 2 s by default,
    /// and unbounded from the user's point of view since it depends on config.
    fn await_response(
        proc: &mut PluginProcess,
        seq: u64,
        timeout: Duration,
        stop: &AtomicBool,
    ) -> Outcome {
        let deadline = Instant::now() + timeout;
        loop {
            if stop.load(Ordering::Relaxed) {
                return Outcome::Stopped;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Outcome::Timeout;
            }
            match proc.recv_timeout(left.min(IDLE_SLICE)) {
                Ok(ReadEvent::Line(line)) => match serde_json::from_str::<PollResponse>(&line) {
                    Ok(resp) if resp.seq != 0 && resp.seq != seq => {
                        log::debug!("discarding plugin reply for seq {} (waiting on {seq})", resp.seq);
                        continue;
                    }
                    Ok(resp) => return Outcome::Response(Box::new(resp)),
                    Err(e) => return Outcome::Protocol(format!("unparseable response: {e}")),
                },
                Ok(ReadEvent::Closed(why)) => return Outcome::Dead(why),
                // A slice elapsed, not the deadline — loop back and re-check.
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Outcome::Dead("plugin reader stopped".into());
                }
            }
        }
    }

    fn apply(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Response(resp) => {
                let mut resp = *resp;
                sanitize::sanitize_response(&mut resp, &self.config.name);

                if self.manifest.is_none()
                    && let Some(m) = resp.manifest.clone()
                {
                    log::info!(
                        "plugin '{}' identified: {} v{}",
                        self.config.name,
                        m.name,
                        m.version
                    );
                    self.manifest = Some(m);
                }

                // A plugin reporting its own failure is unhealthy but still
                // connected — we keep and display whatever it did send.
                let healthy = resp.status.is_empty() || resp.status == "ok";
                if !healthy {
                    log::debug!(
                        "plugin '{}' reported status {:?}",
                        self.config.name,
                        resp.status
                    );
                }
                self.consecutive_errors = 0;
                self.crash_count = 0;

                let display_name = self
                    .manifest
                    .as_ref()
                    .map(|m| m.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| self.config.name.clone());
                let state = if healthy {
                    PluginState::Healthy
                } else {
                    PluginState::Unhealthy
                };
                self.write_status(|s| {
                    s.display_name = display_name;
                    s.state = state;
                    s.response = Some(resp);
                });
            }
            Outcome::Timeout => {
                self.on_soft_error(&format!(
                    "no response within {}ms",
                    self.config.timeout_ms
                ));
            }
            Outcome::Protocol(why) => self.on_soft_error(&why),
            Outcome::Dead(why) => self.on_death(&why),
            Outcome::Stopped => {}
        }
    }

    /// The plugin is alive but not answering usefully. Escalate: warn first,
    /// recycle only if it keeps happening.
    fn on_soft_error(&mut self, why: &str) {
        self.consecutive_errors += 1;
        log::debug!(
            "plugin '{}' poll error ({}/{RESTART_AFTER}): {why}",
            self.config.name,
            self.consecutive_errors
        );
        if self.consecutive_errors >= RESTART_AFTER {
            self.on_death(&format!("{RESTART_AFTER} consecutive failed polls: {why}"));
        } else if self.consecutive_errors >= UNHEALTHY_AFTER {
            self.set_state(PluginState::Unhealthy);
        }
    }

    // --- status plumbing ---

    fn set_state(&self, state: PluginState) {
        self.write_status(|s| s.state = state);
    }

    fn write_status(&self, f: impl FnOnce(&mut PluginStatus)) {
        match self.status.lock() {
            Ok(mut g) => f(&mut g),
            Err(p) => f(&mut p.into_inner()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::protocol::PollRequest;

    /// `PollRequestRef` exists only to avoid cloning the process list. If it
    /// ever drifts from the owned type, plugins would silently start seeing a
    /// different request shape than the protocol crate documents.
    #[test]
    fn request_wire_format_matches_the_owned_type() {
        let processes = vec![
            ProcessContext { pid: 7, name: "ollama".into(), vram_bytes: Some(4096) },
            ProcessContext { pid: 8, name: "字幕".into(), vram_bytes: None },
        ];

        let borrowed = serde_json::to_string(&PollRequestRef {
            msg_type: "poll",
            schema_version: super::super::protocol::SCHEMA_VERSION,
            seq: 42,
            timestamp_ms: 1_700_000_000_000,
            processes: &processes,
        })
        .unwrap();

        let owned = serde_json::to_string(&PollRequest::with_seq(
            1_700_000_000_000,
            42,
            processes.clone(),
        ))
        .unwrap();

        assert_eq!(borrowed, owned);

        // And it still deserializes on the plugin side.
        let round: PollRequest = serde_json::from_str(&borrowed).unwrap();
        assert_eq!(round.msg_type, "poll");
        assert_eq!(round.seq, 42);
        assert_eq!(round.processes.len(), 2);
    }

    /// Backoff must climb and then flatten: a plugin that fails to spawn at
    /// all should settle at one attempt per 30 s, not retry forever at 1 Hz.
    #[test]
    fn backoff_climbs_then_caps() {
        let cfg = PluginConfig {
            name: "t".into(),
            command: "x".into(),
            args: vec![],
            enabled: true,
            timeout_ms: 100,
        };
        let ctx: SharedContext = Arc::new(Mutex::new(Arc::new(Vec::new())));
        let status = Arc::new(Mutex::new(PluginStatus {
            name: "t".into(),
            display_name: "t".into(),
            state: PluginState::Starting,
            response: None,
        }));
        let mut sup = Supervisor::new(cfg, status, ctx);

        let secs: Vec<u64> = (0..8)
            .map(|_| {
                let d = sup.backoff().as_secs();
                sup.crash_count += 1;
                d
            })
            .collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 30, 30, 30]);
    }
}
