use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "dofek", version, about = "Terminal-native system monitor for Windows, Linux, and macOS")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Manage installed plugins (list / add / remove / enable / disable).
    Plugins {
        #[command(subcommand)]
        action: PluginsAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum PluginsAction {
    /// List installed plugins.
    List,
    /// Install a plugin from a local binary path. The binary is copied into
    /// the managed plugin directory, made executable, probed for its manifest,
    /// and registered in the managed plugins.toml — no manual config editing.
    Add {
        /// Path to the plugin binary to install.
        path: PathBuf,
        /// Optional CLI arguments passed to the plugin on every spawn (e.g.
        /// `--host http://localhost:11434`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Uninstall a plugin (removes it from plugins.toml and deletes the binary).
    Remove {
        /// Plugin name as shown by `plugins list`.
        name: String,
    },
    /// Enable a previously installed plugin.
    Enable { name: String },
    /// Disable a plugin without uninstalling it.
    Disable { name: String },
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub lhm: LhmConfig,
    #[serde(default)]
    pub categories: CategoriesConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    #[serde(default, rename = "plugins")]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PluginConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 { 2000 }

#[derive(Deserialize, Debug, Clone)]
pub struct GeneralConfig {
    #[serde(default = "default_refresh_ms")]
    pub refresh_ms: u64,
    #[serde(default = "default_history_len")]
    pub history_len: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DisplayConfig {
    #[serde(default = "default_true")]
    pub show_temps: bool,
    #[serde(default = "default_true")]
    pub show_power: bool,
    #[serde(default = "default_process_count")]
    pub process_count: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AiConfig {
    #[serde(default = "default_vram_threshold")]
    pub vram_threshold_gb: f64,
    #[serde(default = "default_ai_processes")]
    pub known_ai_processes: Vec<String>,
    /// Pre-lowercased version of known_ai_processes (computed at load time).
    #[serde(skip)]
    pub known_ai_lower: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LhmConfig {
    #[serde(default = "default_lhm_url")]
    pub url: String,
}

fn default_refresh_ms() -> u64 { 500 }
fn default_history_len() -> usize { 60 }
fn default_true() -> bool { true }
fn default_process_count() -> usize { 10 }
fn default_vram_threshold() -> f64 { 1.0 }
fn default_lhm_url() -> String { "http://localhost:8085".to_string() }

#[derive(Deserialize, Debug, Clone)]
pub struct TelemetryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_telemetry_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_flush_interval_secs")]
    pub flush_interval_secs: u64,
}

fn default_telemetry_endpoint() -> String { "https://dofek.dev/api/v1/events".to_string() }
fn default_flush_interval_secs() -> u64 { 60 }

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_telemetry_endpoint(),
            flush_interval_secs: default_flush_interval_secs(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CategoriesConfig {
    #[serde(default = "default_dev_processes")]
    pub dev_processes: Vec<String>,
    #[serde(default)]
    pub watch_processes: Vec<String>,
    #[serde(default)]
    pub watch_pids: Vec<u32>,
    /// Pre-lowercased versions (computed at load time).
    #[serde(skip)]
    pub dev_lower: Vec<String>,
    #[serde(skip)]
    pub watch_lower: Vec<String>,
}

impl Default for CategoriesConfig {
    fn default() -> Self {
        let dev = default_dev_processes();
        let dev_lower = dev.iter().map(|s| s.to_lowercase()).collect();
        Self {
            dev_processes: dev,
            watch_processes: Vec::new(),
            watch_pids: Vec::new(),
            dev_lower,
            watch_lower: Vec::new(),
        }
    }
}

fn default_dev_processes() -> Vec<String> {
    vec![
        "code".to_string(),
        "cargo".to_string(),
        "rustc".to_string(),
        "node".to_string(),
        "npm".to_string(),
        "git".to_string(),
        "docker".to_string(),
        "go".to_string(),
    ]
}

fn default_ai_processes() -> Vec<String> {
    vec![
        "ollama".to_string(),
        "ollama_llama_server".to_string(),
        "python".to_string(),
        "lm_studio".to_string(),
        "claude".to_string(),
    ]
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { refresh_ms: default_refresh_ms(), history_len: default_history_len() }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { show_temps: true, show_power: true, process_count: default_process_count() }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        let procs = default_ai_processes();
        let lower = procs.iter().map(|s| s.to_lowercase()).collect();
        Self { vram_threshold_gb: default_vram_threshold(), known_ai_processes: procs, known_ai_lower: lower }
    }
}

impl Default for LhmConfig {
    fn default() -> Self {
        Self { url: default_lhm_url() }
    }
}

impl Config {
    /// Pre-compute lowercased versions of process name lists to avoid per-process allocations.
    pub fn precompute_lowercase(&mut self) {
        self.ai.known_ai_lower = self.ai.known_ai_processes.iter().map(|s| s.to_lowercase()).collect();
        self.categories.dev_lower = self.categories.dev_processes.iter().map(|s| s.to_lowercase()).collect();
        self.categories.watch_lower = self.categories.watch_processes.iter().map(|s| s.to_lowercase()).collect();
    }

    /// Load config from file lookup order:
    /// 1. `--config` CLI flag
    /// 2. `$DOFEK_CONFIG`
    /// 3. user config dir / dofek / dofek.toml
    ///    (Windows: %APPDATA%\dofek\dofek.toml; Linux: ~/.config/dofek/dofek.toml)
    ///
    /// `Config::plugins` here contains *only* the user-owned dofek.toml
    /// entries. Managed plugins (installed via `dofek-tui plugins ...` or the
    /// GUI) live in a separate `<config_dir>/dofek/plugins.toml` and are
    /// composed in by the data collector — that lets the collector watch the
    /// file's mtime and hot-reload plugins without restarting the app.
    pub fn load(cli: &Cli) -> Result<Self> {
        let candidates = config_candidates(
            cli.config.as_deref(),
            std::env::var_os("DOFEK_CONFIG").map(PathBuf::from).as_deref(),
            dirs::config_dir().as_deref(),
        );

        let mut config = Config::default();
        let mut loaded_from = None;
        for path in &candidates {
            if path.exists() {
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read config from {}", path.display()))?;
                config = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse config from {}", path.display()))?;
                loaded_from = Some(path.clone());
                break;
            }
        }

        config.precompute_lowercase();
        config.validate();
        match loaded_from {
            Some(path) => log::info!("Loaded config from {}", path.display()),
            None => log::info!("No dofek.toml found, using defaults"),
        }
        Ok(config)
    }

    /// Sanity-check values that came from a file, repairing anything unsafe.
    ///
    /// Only the LHM URL needs this today: it is fed to `ureq::get` on every
    /// poll, and a config file is not necessarily written by the person
    /// running dofek.
    fn validate(&mut self) {
        if let Err(reason) = validate_lhm_url(&self.lhm.url) {
            log::warn!(
                "Ignoring lhm.url {:?} ({reason}) — falling back to {}",
                self.lhm.url,
                default_lhm_url()
            );
            self.lhm.url = default_lhm_url();
        }
    }
}

/// The config-file search order, as data.
///
/// Split out of [`Config::load`] so the ordering is testable without touching
/// the process's working directory or environment.
///
/// **`./dofek.toml` is deliberately absent.** It used to come first, which
/// meant `cd`-ing into a directory containing a hostile `dofek.toml` and
/// running dofek would load it — and a config file can declare `[[plugins]]`,
/// which dofek spawns as child processes. Set `$DOFEK_CONFIG` (or pass
/// `--config`) to opt into a project-local config explicitly.
pub fn config_candidates(
    cli_config: Option<&Path>,
    env_config: Option<&Path>,
    config_dir: Option<&Path>,
) -> Vec<PathBuf> {
    // An explicit flag wins outright: the user named this file on the command
    // line, so don't silently fall back to anything else if it's missing.
    if let Some(path) = cli_config {
        return vec![path.to_path_buf()];
    }
    let mut paths = Vec::new();
    if let Some(path) = env_config {
        paths.push(path.to_path_buf());
    }
    if let Some(dir) = config_dir {
        paths.push(dir.join("dofek").join("dofek.toml"));
    }
    paths
}

/// Reject LHM URLs that aren't plainly a local HTTP endpoint.
///
/// Returns `Err(reason)` describing the first problem found.
pub fn validate_lhm_url(url: &str) -> Result<(), &'static str> {
    if url.is_empty() {
        return Err("empty");
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("contains control or whitespace characters");
    }
    let rest = match url.split_once("://") {
        Some(("http", rest)) | Some(("https", rest)) => rest,
        Some(_) => return Err("scheme must be http or https"),
        None => return Err("missing scheme"),
    };
    if rest.is_empty() {
        return Err("missing host");
    }
    // Credentials in the URL would be sent on every poll, to whatever host
    // follows the `@`. Nothing legitimate needs them here.
    if rest.split('/').next().unwrap_or("").contains('@') {
        return Err("must not embed credentials");
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or_else(|| rest.split(['/', '?', '#']).next().unwrap_or(""));
    // Not an error — LHM is documented as a localhost service, but pointing it
    // at another machine on a trusted LAN is a legitimate (if unusual) setup.
    if !matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]") {
        log::warn!("lhm.url points at non-loopback host {host:?} — sensor data will be fetched over the network");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression this ordering exists to prevent: a `dofek.toml` sitting
    /// in the current directory must never be picked up implicitly. A config
    /// file can declare `[[plugins]]`, which dofek spawns as child processes,
    /// so `cd`-ing into a hostile directory used to be enough to get code run.
    #[test]
    fn cwd_config_is_never_a_candidate() {
        let cfg = Path::new("/home/u/.config");
        let candidates = config_candidates(None, None, Some(cfg));
        for c in &candidates {
            assert!(
                c.is_absolute(),
                "candidate {c:?} is relative, so it resolves against the CWD"
            );
        }
        assert!(!candidates.contains(&PathBuf::from("dofek.toml")));
        assert!(!candidates.contains(&PathBuf::from("./dofek.toml")));
    }

    #[test]
    fn default_order_is_env_then_config_dir() {
        let cfg = Path::new("/home/u/.config");
        let env = Path::new("/projects/app/dofek.toml");

        assert_eq!(
            config_candidates(None, Some(env), Some(cfg)),
            vec![
                PathBuf::from("/projects/app/dofek.toml"),
                PathBuf::from("/home/u/.config/dofek/dofek.toml"),
            ]
        );

        // Without the env var, only the user config dir.
        assert_eq!(
            config_candidates(None, None, Some(cfg)),
            vec![PathBuf::from("/home/u/.config/dofek/dofek.toml")]
        );
    }

    /// `--config` is an explicit instruction, so it must not silently fall
    /// back to another file if it doesn't exist.
    #[test]
    fn cli_flag_wins_outright() {
        let cli = Path::new("/tmp/explicit.toml");
        assert_eq!(
            config_candidates(Some(cli), Some(Path::new("/env.toml")), Some(Path::new("/cfg"))),
            vec![PathBuf::from("/tmp/explicit.toml")]
        );
    }

    #[test]
    fn no_config_dir_is_not_a_panic() {
        assert!(config_candidates(None, None, None).is_empty());
        assert_eq!(
            config_candidates(None, Some(Path::new("/env.toml")), None),
            vec![PathBuf::from("/env.toml")]
        );
    }

    #[test]
    fn accepts_normal_lhm_urls() {
        for url in [
            "http://localhost:8085",
            "http://127.0.0.1:8085",
            "https://localhost:8085/",
            "http://[::1]:8085",
            "http://192.168.1.50:8085", // non-loopback: warns, but allowed
        ] {
            assert!(validate_lhm_url(url).is_ok(), "should accept {url}: {:?}", validate_lhm_url(url));
        }
    }

    #[test]
    fn rejects_dangerous_lhm_urls() {
        for (url, why) in [
            ("", "empty"),
            ("localhost:8085", "missing scheme"),
            ("file:///etc/passwd", "scheme must be http or https"),
            ("ftp://host/x", "scheme must be http or https"),
            ("http://", "missing host"),
            ("http://user:pw@evil.example", "must not embed credentials"),
            ("http://localhost:8085\nX-Injected: 1", "contains control or whitespace characters"),
            ("http://local host:8085", "contains control or whitespace characters"),
        ] {
            assert_eq!(validate_lhm_url(url), Err(why), "for {url:?}");
        }
    }

    /// A bad URL in a config file degrades to the default rather than
    /// disabling LHM or failing the whole load.
    #[test]
    fn bad_lhm_url_falls_back_to_the_default() {
        let mut cfg = Config::default();
        cfg.lhm.url = "file:///etc/passwd".to_string();
        cfg.validate();
        assert_eq!(cfg.lhm.url, default_lhm_url());

        let mut ok = Config::default();
        ok.lhm.url = "http://127.0.0.1:9999".to_string();
        ok.validate();
        assert_eq!(ok.lhm.url, "http://127.0.0.1:9999");
    }
}
