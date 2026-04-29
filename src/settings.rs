use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// User UI preferences persisted across sessions.
/// Stored in `%APPDATA%/dofek/settings.toml`, separate from the system config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub chart_tab: String,
    pub chart_mode: String,
    pub sort_column: String,
    pub sort_ascending: bool,
    pub category_filter: String,
    pub split_pct: u16,
    pub refresh_ms: u64,
    #[serde(default = "generate_anonymous_id")]
    pub anonymous_id: String,
    /// Whether the user has been asked about telemetry (first-run prompt).
    #[serde(default)]
    pub telemetry_prompted: bool,
    /// User's telemetry choice (overrides config file when `telemetry_prompted` is true).
    #[serde(default)]
    pub telemetry_enabled: bool,
    /// Show a system-tray / menu-bar icon with a live CPU sparkline.
    #[serde(default = "default_true")]
    pub enable_tray: bool,
    /// Pressing the window close button hides to tray instead of quitting.
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// Launch with the main window hidden (tray-only).
    #[serde(default)]
    pub start_in_tray: bool,
    /// On macOS, render `CPU NN GPU NN` text next to the menu-bar icon.
    /// Superseded by `tray_display_mode`; kept for backwards compat with
    /// existing settings.toml files. New code reads `tray_display_mode`.
    #[serde(default = "default_true")]
    pub tray_show_text: bool,
    /// What the tray entry shows: `"chart"`, `"chart+text"`, or `"text"`.
    /// On Windows/Linux the text portion is a no-op (system trays there
    /// don't carry a title), so `"text"` falls back to chart-only behavior.
    #[serde(default = "default_tray_display_mode")]
    pub tray_display_mode: String,
    /// Run a background "is there a newer Dofek?" check on app launch and
    /// surface the result if a newer release exists. Off by default — this is
    /// an opt-in network call.
    #[serde(default)]
    pub check_updates_on_startup: bool,
}

fn generate_anonymous_id() -> String {
    Uuid::new_v4().to_string()
}

fn default_true() -> bool {
    true
}

fn default_tray_display_mode() -> String {
    "chart+text".to_string()
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            chart_tab: "cpu".to_string(),
            chart_mode: "default".to_string(),
            sort_column: "memory".to_string(),
            sort_ascending: false,
            category_filter: "all".to_string(),
            split_pct: 58,
            refresh_ms: 500,
            anonymous_id: generate_anonymous_id(),
            telemetry_prompted: false,
            telemetry_enabled: false,
            enable_tray: true,
            close_to_tray: true,
            start_in_tray: false,
            tray_show_text: true,
            tray_display_mode: default_tray_display_mode(),
            check_updates_on_startup: false,
        }
    }
}

impl UserSettings {
    /// Canonical path: `%APPDATA%/dofek/settings.toml`
    pub fn file_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dofek")
            .join("settings.toml")
    }

    /// Load from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(s) => s,
            Err(e) => {
                log::info!("Using default settings: {e}");
                Self::default()
            }
        }
    }

    fn try_load() -> Result<Self> {
        let path = Self::file_path();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let settings: Self = toml::from_str(&content)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(settings)
    }

    /// Save to disk, creating the directory if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::file_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        let content = toml::to_string_pretty(self)
            .context("serializing settings")?;
        std::fs::write(&path, content)
            .with_context(|| format!("writing {}", path.display()))?;
        log::info!("Settings saved to {}", path.display());
        Ok(())
    }
}
