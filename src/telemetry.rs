//! Opt-in usage telemetry for beta feedback.
//!
//! Events are batched in memory and flushed via HTTP POST on a timer.
//! Disabled by default — users opt in via `[telemetry] enabled = true` in dofek.toml.

use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Telemetry event variants. Serialized as `{"event": "snake_case", ...fields}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TelemetryEvent {
    SessionStart {
        interface: String,
        app_version: String,
        os_version: String,
    },
    SessionEnd {
        duration_secs: u64,
    },
    GpuPath {
        path: String,
        device_count: usize,
        device_names: Vec<String>,
    },
    TabSwitch {
        tab: String,
    },
    ChartModeToggle {
        mode: String,
    },
    FilterChange {
        filter: String,
    },
    PanelSwitch {
        panel: String,
    },
    PluginUsed {
        plugin_name: String,
        state: String,
    },
    Heartbeat {
        current_tab: String,
        process_count: usize,
        plugin_count: usize,
    },
    /// Internal sentinel — triggers flush and thread exit. Never serialized over the wire.
    #[serde(skip)]
    Shutdown,
}

/// Lightweight handle for emitting telemetry events.
/// Clone is cheap (wraps an `Option<mpsc::Sender>`). When telemetry is disabled
/// the inner is `None` and all `track()` calls are no-ops.
#[derive(Clone)]
pub struct TelemetryHandle {
    tx: Option<mpsc::Sender<TelemetryEvent>>,
}

impl TelemetryHandle {
    /// Returns a no-op handle (telemetry disabled).
    pub fn disabled() -> Self {
        Self { tx: None }
    }

    /// Queue an event for batching. Silently drops if disabled or channel closed.
    pub fn track(&self, event: TelemetryEvent) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(event);
        }
    }

    /// Signal the flush thread to send remaining events and exit.
    pub fn shutdown(&self) {
        self.track(TelemetryEvent::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TelemetryBatch {
    anonymous_id: String,
    batch: Vec<TelemetryEnvelope>,
}

#[derive(Serialize)]
struct TelemetryEnvelope {
    timestamp_ms: u64,
    #[serde(flatten)]
    event: TelemetryEvent,
}

// ---------------------------------------------------------------------------
// Spawn & flush
// ---------------------------------------------------------------------------

/// Spawn the telemetry background thread. Returns a handle for emitting events.
///
/// If `enabled` is false, returns a no-op handle and no thread is spawned.
/// Accepts primitives rather than a config struct to avoid type-identity issues
/// between the binary's `mod config` and the library's `pub mod config`.
pub fn spawn_telemetry(
    enabled: bool,
    endpoint: &str,
    flush_interval_secs: u64,
    anonymous_id: String,
) -> TelemetryHandle {
    if !enabled {
        log::info!("Telemetry disabled");
        return TelemetryHandle::disabled();
    }

    log::info!("Telemetry enabled — endpoint: {endpoint}");

    let (tx, rx) = mpsc::channel();
    let endpoint = endpoint.to_string();
    let flush_interval = Duration::from_secs(flush_interval_secs);
    let anon_id = anonymous_id;

    thread::Builder::new()
        .name("telemetry-flush".into())
        .spawn(move || {
            flush_loop(&rx, &endpoint, &anon_id, flush_interval);
        })
        .expect("failed to spawn telemetry thread");

    TelemetryHandle { tx: Some(tx) }
}

fn flush_loop(
    rx: &mpsc::Receiver<TelemetryEvent>,
    endpoint: &str,
    anonymous_id: &str,
    flush_interval: Duration,
) {
    let mut batch: Vec<TelemetryEnvelope> = Vec::new();
    let mut last_flush = Instant::now();

    loop {
        let remaining = flush_interval.saturating_sub(last_flush.elapsed());

        match rx.recv_timeout(remaining) {
            Ok(TelemetryEvent::Shutdown) => {
                flush(endpoint, anonymous_id, &mut batch);
                return;
            }
            Ok(event) => {
                batch.push(TelemetryEnvelope {
                    timestamp_ms: now_ms(),
                    event,
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Interval elapsed — flush below.
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // All senders dropped — flush remaining and exit.
                flush(endpoint, anonymous_id, &mut batch);
                return;
            }
        }

        if last_flush.elapsed() >= flush_interval && !batch.is_empty() {
            flush(endpoint, anonymous_id, &mut batch);
            last_flush = Instant::now();
        }
    }
}

/// POST batch to endpoint. On failure, silently drops events (offline-safe).
fn flush(endpoint: &str, anonymous_id: &str, batch: &mut Vec<TelemetryEnvelope>) {
    if batch.is_empty() {
        return;
    }

    let payload = TelemetryBatch {
        anonymous_id: anonymous_id.to_string(),
        batch: std::mem::take(batch),
    };

    let count = payload.batch.len();

    let json = match serde_json::to_string(&payload) {
        Ok(j) => j,
        Err(e) => {
            log::debug!("Telemetry serialize error: {e}");
            return;
        }
    };

    match ureq::post(endpoint)
        .timeout(Duration::from_secs(5))
        .set("Content-Type", "application/json")
        .send_string(&json)
    {
        Ok(_) => log::debug!("Telemetry: flushed {count} events"),
        Err(e) => log::debug!("Telemetry flush failed (dropping batch): {e}"),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
