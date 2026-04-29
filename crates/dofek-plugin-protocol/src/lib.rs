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

// --- Request (Dofek -> plugin) ---

/// Sent on every refresh cycle. Plugins should respond within `timeout_ms`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PollRequest {
    #[serde(rename = "type")]
    pub msg_type: String,
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
}

impl PollRequest {
    pub fn new(timestamp_ms: u64, processes: Vec<ProcessContext>) -> Self {
        Self {
            msg_type: "poll".to_string(),
            timestamp_ms,
            processes,
        }
    }
}

impl Default for ShutdownRequest {
    fn default() -> Self {
        Self {
            msg_type: "shutdown".to_string(),
        }
    }
}

impl ShutdownRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

// --- Response (plugin -> Dofek) ---

/// Plugin response to a [`PollRequest`]. All fields except `status` are optional.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PollResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PluginManifest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panels: Vec<Panel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_annotations: Vec<ProcessAnnotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<Metric>,
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
