//! Dofek plugin protocol — JSON-over-stdio message types shared between Dofek
//! and external plugins.
//!
//! Plugins read newline-delimited [`PollRequest`] / [`ShutdownRequest`] objects
//! on stdin and write [`PollResponse`] objects on stdout. See the protocol
//! reference at <https://dofek.dev/plugins/> for details.
//!
//! All types implement both `Serialize` and `Deserialize` so this crate can be
//! used from either side of the protocol.

use serde::{Deserialize, Serialize};

/// Version of the JSON message shapes in this module.
///
/// Dofek stamps it on every [`PollRequest`]; plugins may echo it back on a
/// [`PollResponse`]. Both sides default it to `1` when it is absent, so a
/// plugin written before the field existed keeps working unchanged.
pub const SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

// --- Request (Dofek -> plugin) ---

/// Sent on every refresh cycle. Plugins should respond within `timeout_ms`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PollRequest {
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Message-shape version — see [`SCHEMA_VERSION`].
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Monotonic per-plugin request counter. A plugin that echoes it back on
    /// its [`PollResponse`] lets Dofek discard a reply that arrives after its
    /// request already timed out, instead of misreading it as the answer to
    /// the *next* poll. Echoing is optional: `0` means "unknown", and Dofek
    /// then falls back to first-reply-wins.
    #[serde(default)]
    pub seq: u64,
    pub timestamp_ms: u64,
    #[serde(default)]
    pub processes: Vec<ProcessContext>,
}

/// One process snapshot delivered to plugins each poll.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessContext {
    pub pid: u32,
    pub name: String,
    #[serde(default)]
    pub vram_bytes: Option<u64>,
}

/// Sent once when Dofek is exiting. Plugins should clean up and exit.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShutdownRequest {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

impl PollRequest {
    pub fn new(timestamp_ms: u64, processes: Vec<ProcessContext>) -> Self {
        Self {
            msg_type: "poll".to_string(),
            schema_version: SCHEMA_VERSION,
            seq: 0,
            timestamp_ms,
            processes,
        }
    }

    /// Same as [`PollRequest::new`] but stamped with a request counter.
    pub fn with_seq(timestamp_ms: u64, seq: u64, processes: Vec<ProcessContext>) -> Self {
        Self {
            seq,
            ..Self::new(timestamp_ms, processes)
        }
    }
}

impl Default for ShutdownRequest {
    fn default() -> Self {
        Self {
            msg_type: "shutdown".to_string(),
            schema_version: SCHEMA_VERSION,
        }
    }
}

impl ShutdownRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

// --- Response (plugin -> Dofek) ---

/// Plugin response to a [`PollRequest`]. Every field is optional.
///
/// `status` is honored: any value other than `"ok"` / `""` marks the plugin
/// unhealthy in the UI while still displaying whatever data it did send.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PollResponse {
    #[serde(default)]
    pub status: String,
    /// Message-shape version — see [`SCHEMA_VERSION`]. Absent ⇒ `1`.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Echo of [`PollRequest::seq`]. Optional; `0` means "not echoed".
    #[serde(default)]
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PluginManifest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panels: Vec<Panel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_annotations: Vec<ProcessAnnotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<Metric>,
}

/// Hand-written rather than derived so `..Default::default()` in a plugin
/// still stamps the current [`SCHEMA_VERSION`]. `#[serde(default = ...)]` only
/// covers *de*serialization, so a derived `Default` would emit
/// `"schema_version": 0` on the wire.
impl Default for PollResponse {
    fn default() -> Self {
        Self {
            status: String::new(),
            schema_version: SCHEMA_VERSION,
            seq: 0,
            manifest: None,
            panels: Vec::new(),
            process_annotations: Vec::new(),
            metrics: Vec::new(),
        }
    }
}

/// Plugin self-identification, sent in the first [`PollResponse`] only.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

/// Key-value panel rendered in the plugin dock at the bottom of the watchlist.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Panel {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub content: Vec<PanelEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PanelEntry {
    pub key: String,
    pub value: String,
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_style() -> String {
    "normal".to_string()
}

/// Annotation that overrides or augments a process row in the watchlist.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessAnnotation {
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_state: Option<String>,
}

/// Named numeric value displayed as a pill in the top ticker bar.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metric {
    pub id: String,
    pub label: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
}
