use serde::{Deserialize, Serialize};

// --- Request (dofek -> plugin) ---

#[derive(Serialize, Debug)]
pub struct PollRequest {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub timestamp_ms: u64,
    pub processes: Vec<ProcessContext>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ProcessContext {
    pub pid: u32,
    pub name: String,
    pub vram_bytes: Option<u64>,
}

#[derive(Serialize, Debug)]
pub struct ShutdownRequest {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
}

impl PollRequest {
    pub fn new(timestamp_ms: u64, processes: Vec<ProcessContext>) -> Self {
        Self {
            msg_type: "poll",
            timestamp_ms,
            processes,
        }
    }
}

impl ShutdownRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ShutdownRequest {
    fn default() -> Self {
        Self {
            msg_type: "shutdown",
        }
    }
}

// --- Response (plugin -> dofek) ---

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PollResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub manifest: Option<PluginManifest>,
    #[serde(default)]
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub process_annotations: Vec<ProcessAnnotation>,
    #[serde(default)]
    pub metrics: Vec<Metric>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Panel {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub content: Vec<PanelEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PanelEntry {
    pub key: String,
    pub value: String,
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_style() -> String {
    "normal".to_string()
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProcessAnnotation {
    pub pid: u32,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub ai_state: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Metric {
    pub id: String,
    pub label: String,
    pub value: f64,
    #[serde(default)]
    pub unit: String,
}
