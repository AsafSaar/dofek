pub mod process;
pub mod protocol;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::PluginConfig;
use process::PluginProcess;
use protocol::{PollRequest, PollResponse, ProcessContext};

#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Runtime state for a single plugin instance.
struct PluginInstance {
    config: PluginConfig,
    process: Option<PluginProcess>,
    state: PluginState,
    last_response: Option<PollResponse>,
    manifest: Option<protocol::PluginManifest>,
    consecutive_errors: u32,
    crash_count: u32,
    last_crash: Option<Instant>,
}

impl PluginInstance {
    fn new(config: PluginConfig) -> Self {
        Self {
            config,
            process: None,
            state: PluginState::Starting,
            last_response: None,
            manifest: None,
            consecutive_errors: 0,
            crash_count: 0,
            last_crash: None,
        }
    }

    fn backoff_duration(&self) -> Duration {
        let secs = match self.crash_count {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 16,
            _ => 30,
        };
        Duration::from_secs(secs)
    }

    fn should_respawn(&self) -> bool {
        match self.last_crash {
            Some(t) => t.elapsed() >= self.backoff_duration(),
            None => true,
        }
    }

    fn spawn(&mut self) {
        match PluginProcess::spawn(&self.config.command, &self.config.args) {
            Ok(proc) => {
                self.process = Some(proc);
                self.state = PluginState::Starting;
                self.consecutive_errors = 0;
                log::info!("Plugin '{}' spawned (command: {})", self.config.name, self.config.command);
            }
            Err(e) => {
                log::error!("Failed to spawn plugin '{}': {e}", self.config.name);
                self.state = PluginState::Crashed;
                self.crash_count += 1;
                self.last_crash = Some(Instant::now());
            }
        }
    }

    fn poll(&mut self, request: &PollRequest) {
        let proc = match self.process.as_mut() {
            Some(p) => p,
            None => return,
        };

        // Check if process is still alive
        if !proc.is_alive() {
            log::warn!("Plugin '{}' process died", self.config.name);
            self.process = None;
            self.state = PluginState::Crashed;
            self.crash_count += 1;
            self.last_crash = Some(Instant::now());
            return;
        }

        let timeout = Duration::from_millis(self.config.timeout_ms);
        match proc.poll(request, timeout) {
            Ok(response) => {
                // Capture manifest on first successful response
                if self.manifest.is_none()
                    && let Some(ref manifest) = response.manifest
                {
                    log::info!(
                        "Plugin '{}' identified: {} v{}",
                        self.config.name,
                        manifest.name,
                        manifest.version
                    );
                    self.manifest = Some(manifest.clone());
                }

                self.last_response = Some(response);
                self.consecutive_errors = 0;
                self.state = PluginState::Healthy;
            }
            Err(e) => {
                self.consecutive_errors += 1;
                log::debug!(
                    "Plugin '{}' poll error ({}/5): {e}",
                    self.config.name,
                    self.consecutive_errors
                );
                if self.consecutive_errors >= 5 {
                    self.state = PluginState::Unhealthy;
                }
            }
        }
    }

    fn shutdown(&mut self) {
        if let Some(ref mut proc) = self.process {
            proc.send_shutdown();
        }
    }

    fn kill(&mut self) {
        if let Some(ref mut proc) = self.process {
            proc.kill();
        }
        self.process = None;
    }
}

/// Summary of a plugin's current state, sent to the UI layer.
#[derive(Debug, Clone)]
pub struct PluginStatus {
    pub name: String,
    pub display_name: String,
    pub state: PluginState,
    pub response: Option<PollResponse>,
}

/// Manages all plugin instances: spawn, poll, restart, shutdown.
pub struct PluginManager {
    plugins: Vec<PluginInstance>,
}

impl PluginManager {
    /// Create a new PluginManager from config. Spawns all enabled plugins.
    pub fn new(configs: &[PluginConfig]) -> Self {
        let mut plugins: Vec<PluginInstance> = configs
            .iter()
            .filter(|c| c.enabled)
            .map(|c| PluginInstance::new(c.clone()))
            .collect();

        // Initial spawn
        for plugin in &mut plugins {
            plugin.spawn();
        }

        Self { plugins }
    }

    /// Poll all plugins with the current process context. Call this once per refresh cycle.
    pub fn poll_all(&mut self, processes: &[ProcessContext]) -> Vec<PluginStatus> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let request = PollRequest::new(timestamp_ms, processes.to_vec());

        for plugin in &mut self.plugins {
            match plugin.state {
                PluginState::Crashed if plugin.should_respawn() => {
                    log::info!("Respawning plugin '{}' (attempt {})", plugin.config.name, plugin.crash_count + 1);
                    plugin.spawn();
                    if plugin.process.is_some() {
                        plugin.poll(&request);
                    }
                }
                PluginState::Starting | PluginState::Healthy | PluginState::Unhealthy => {
                    plugin.poll(&request);
                }
                _ => {} // Crashed, waiting for backoff
            }
        }

        self.statuses()
    }

    /// Get current status of all plugins.
    fn statuses(&self) -> Vec<PluginStatus> {
        self.plugins
            .iter()
            .map(|p| {
                let display_name = p
                    .manifest
                    .as_ref()
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| p.config.name.clone());
                PluginStatus {
                    name: p.config.name.clone(),
                    display_name,
                    state: p.state,
                    response: p.last_response.clone(),
                }
            })
            .collect()
    }

    /// Graceful shutdown: send shutdown message, wait briefly, then kill.
    pub fn shutdown(&mut self) {
        for plugin in &mut self.plugins {
            plugin.shutdown();
        }

        // Give plugins 2 seconds to exit gracefully
        std::thread::sleep(Duration::from_secs(2));

        for plugin in &mut self.plugins {
            plugin.kill();
        }
    }

    /// Returns true if any plugins are configured.
    pub fn has_plugins(&self) -> bool {
        !self.plugins.is_empty()
    }
}
