use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "dofek", version, about = "Terminal-native system monitor for Windows")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Clone)]
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

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            display: DisplayConfig::default(),
            ai: AiConfig::default(),
            lhm: LhmConfig::default(),
            categories: CategoriesConfig::default(),
            plugins: Vec::new(),
        }
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
    /// 1. --config CLI flag
    /// 2. ./dofek.toml
    /// 3. %APPDATA%/dofek/dofek.toml
    pub fn load(cli: &Cli) -> Result<Self> {
        let candidates: Vec<PathBuf> = if let Some(ref path) = cli.config {
            vec![path.clone()]
        } else {
            let mut paths = vec![PathBuf::from("dofek.toml")];
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join("dofek").join("dofek.toml"));
            }
            paths
        };

        for path in &candidates {
            if path.exists() {
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read config from {}", path.display()))?;
                let mut config: Config = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse config from {}", path.display()))?;
                config.precompute_lowercase();
                log::info!("Loaded config from {}", path.display());
                return Ok(config);
            }
        }

        log::info!("No config file found, using defaults");
        Ok(Config::default())
    }
}
