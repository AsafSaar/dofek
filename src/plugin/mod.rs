//! Plugin system: spawn, supervise, poll and tear down JSON-over-stdio child
//! processes.
//!
//! The runtime is split three ways so that nothing a plugin does can reach the
//! collector thread:
//!
//! * [`process`] owns the OS contract — spawning contained, bounded reads off a
//!   dedicated thread, draining stderr, killing a process group.
//! * [`worker`] owns one plugin's lifecycle — cadence, timeout, health,
//!   backoff — on a supervisor thread of its own.
//! * [`sanitize`] bounds every string and collection a plugin sends, at ingest.
//!
//! [`PluginManager`] is only a fan-out point: it swaps the shared process
//! context, nudges each supervisor, and reads back the last-known status. It
//! never waits on a plugin.

pub mod cli;
pub mod process;
pub mod protocol;
pub mod sanitize;
pub mod store;
pub mod worker;

use std::sync::{Arc, Mutex};

use crate::config::PluginConfig;
use protocol::{PollResponse, ProcessContext};
use worker::{PluginWorker, SharedContext};

/// Serialized lowercase so the JSON matches the `Display` impl (and the
/// protocol's own `status` strings) rather than the Rust variant spelling —
/// the GUI keys its dot colours off these values.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    Starting,
    Healthy,
    Unhealthy,
    Crashed,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginState::Starting => write!(f, "starting"),
            PluginState::Healthy => write!(f, "healthy"),
            PluginState::Unhealthy => write!(f, "unhealthy"),
            PluginState::Crashed => write!(f, "crashed"),
        }
    }
}

/// Summary of a plugin's current state, sent to the UI layer.
///
/// Serialized into `DataSnapshot` for the GUI. Safe to expose because the
/// contained `PollResponse` has already been through `sanitize::response` at
/// ingest — bounded panels/entries/metrics, capped string lengths, control
/// characters stripped — so the payload is a few KB regardless of what the
/// plugin sent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginStatus {
    pub name: String,
    pub display_name: String,
    pub state: PluginState,
    pub response: Option<PollResponse>,
}

/// Owns one supervisor per configured plugin.
pub struct PluginManager {
    workers: Vec<PluginWorker>,
    ctx: SharedContext,
}

impl PluginManager {
    /// Start a supervisor for every enabled plugin in `configs`.
    ///
    /// Returns immediately — each child is spawned on its own supervisor
    /// thread. Previously this spawned every plugin inline, so a plugin whose
    /// binary was on a slow or missing network path delayed dofek's startup.
    pub fn new(configs: &[PluginConfig]) -> Self {
        let ctx: SharedContext = Arc::new(Mutex::new(Arc::new(Vec::new())));
        let workers = configs
            .iter()
            .filter(|c| c.enabled)
            .map(|c| PluginWorker::start(c.clone(), Arc::clone(&ctx)))
            .collect();
        Self { workers, ctx }
    }

    /// Publish this cycle's process list and collect what every plugin last
    /// reported. Call once per refresh.
    ///
    /// Non-blocking by construction: one lock to swap the shared context, one
    /// per plugin to read its status, and a `try_send` nudge that is dropped if
    /// that supervisor is still busy. Nothing here waits on a child process, so
    /// a hung plugin costs the collector nothing.
    ///
    /// Takes the list by value so the common case is zero clones: the collector
    /// builds it once, and every supervisor borrows from the same `Arc`.
    pub fn tick(&self, processes: Vec<ProcessContext>) -> Vec<PluginStatus> {
        if self.workers.is_empty() {
            return Vec::new();
        }
        let shared = Arc::new(processes);
        match self.ctx.lock() {
            Ok(mut g) => *g = shared,
            Err(p) => *p.into_inner() = shared,
        }
        for w in &self.workers {
            w.nudge();
        }
        self.workers.iter().map(PluginWorker::status).collect()
    }

    /// Stop every plugin. Signals all supervisors first, then joins them, so
    /// teardown costs roughly the slowest plugin rather than the sum — the old
    /// implementation slept a flat 2 seconds on the collector thread.
    pub fn shutdown(&mut self) {
        for w in &mut self.workers {
            w.stop_signal();
        }
        for w in &mut self.workers {
            w.join();
        }
        self.workers.clear();
    }

    /// Swap the plugin set in place. Used when the user installs, removes or
    /// toggles a plugin via the GUI or `dofek-tui plugins ...` and
    /// `plugins.toml` changes on disk.
    pub fn replace(&mut self, configs: &[PluginConfig]) {
        self.shutdown();
        *self = PluginManager::new(configs);
    }

    /// Whether any plugin is configured. The collector checks this before
    /// building the per-tick process context at all — that context is ~500
    /// `String` clones a second, and until now it was built on every tick of
    /// every install regardless of whether a plugin existed to read it.
    pub fn has_plugins(&self) -> bool {
        !self.workers.is_empty()
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}
