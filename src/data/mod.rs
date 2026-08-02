pub mod ai_detect;
pub mod disk;
pub mod gpu;
pub mod lhm;
pub mod network;
pub mod process;
#[cfg(target_os = "linux")]
pub mod rapl;
pub mod rate;
pub mod sysinfo_source;

use crate::config::{Config, PluginConfig};
use crate::plugin::{PluginManager, PluginStatus};
use crate::plugin::protocol::ProcessContext;
use crate::plugin::store::PluginStore;
use disk::{DiskStats, DiskTracker};
use lhm::{CpuSensors, GpuSensors, MemorySensors};
use network::{NetworkStats, NetworkTracker};
use process::ProcessInfo;
use gpu::NvmlState;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::System;

/// Complete snapshot of all system data at a point in time.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DataSnapshot {
    pub cpu: CpuSensors,
    pub memory: MemorySensors,
    pub gpus: Vec<GpuSensors>,
    pub network: NetworkStats,
    pub disk: DiskStats,
    pub processes: Vec<ProcessInfo>,
    pub nvml_available: bool,
    pub lhm_connected: bool,
    pub hostname: String,
    #[serde(skip)]
    pub timestamp: Instant,
    /// Serialized since v1.7 so the GUI can render a live plugin dock. The
    /// payload is bounded at ingest by `plugin::sanitize`, and every GUI sink
    /// that touches it writes through `textContent`.
    pub plugin_statuses: Vec<PluginStatus>,
}

impl Default for DataSnapshot {
    fn default() -> Self {
        Self {
            cpu: CpuSensors::default(),
            memory: MemorySensors::default(),
            gpus: Vec::new(),
            network: NetworkStats::default(),
            disk: DiskStats::default(),
            processes: Vec::new(),
            nvml_available: false,
            lhm_connected: false,
            hostname: System::host_name().unwrap_or_default(),
            timestamp: Instant::now(),
            plugin_statuses: Vec::new(),
        }
    }
}

/// Returns the modification time of `path` as a `SystemTime`, or `None` if
/// the path is missing or its metadata can't be read. We only use the value
/// for equality comparison ("did this file change since last tick?"), so a
/// missing-file `None` is a meaningful state, not an error.
fn read_mtime(path: Option<&std::path::Path>) -> Option<std::time::SystemTime> {
    let p = path?;
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}

/// Handle for stopping the collector thread and waiting for it to finish.
///
/// Needed because plugin children are put in their own session
/// (`plugin::process`), which is what lets dofek reap a plugin's own
/// grandchildren — but also means they no longer receive the terminal's
/// job-control signals. Without an explicit teardown, quitting dofek would
/// leave every plugin running. The collector owns the `PluginManager`, so this
/// is the only place that teardown can be triggered from.
pub struct CollectorHandle {
    stop: Arc<std::sync::atomic::AtomicBool>,
    /// Dropping this wakes the collector out of its inter-tick sleep, so
    /// shutdown costs one in-flight tick rather than a full refresh interval.
    wake: Option<mpsc::Sender<()>>,
    join: Option<thread::JoinHandle<()>>,
}

impl CollectorHandle {
    /// Stop collecting and tear down every plugin. Bounded: one in-flight tick
    /// plus the plugin grace period (plugins tear down concurrently).
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake = None;
        if let Some(h) = self.join.take()
            && h.join().is_err()
        {
            log::warn!("collector thread panicked during shutdown");
        }
    }
}

impl Drop for CollectorHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn the data collector thread. Returns a receiver for snapshots and a
/// handle for shutting it down.
///
/// The polling interval is read from `refresh_ms` on every loop iteration so
/// runtime changes (TUI `+`/`-` keys) take effect on the next sleep without
/// needing to respawn the thread. `config.general.refresh_ms` is ignored —
/// the caller is responsible for seeding the atomic from it.
pub fn spawn_collector(
    config: Config,
    refresh_ms: Arc<AtomicU64>,
) -> (mpsc::Receiver<DataSnapshot>, CollectorHandle) {
    let (tx, rx) = mpsc::channel();
    let (wake, wake_rx) = mpsc::channel::<()>();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);

    let join = thread::spawn(move || {
        let stop = thread_stop;
        let mut net_tracker = NetworkTracker::default();
        let mut disk_tracker = DiskTracker::default();
        let nvml = NvmlState::init();
        let mut prev_vram: HashMap<u32, u64> = HashMap::new();
        let mut lhm_failed = false; // stop retrying LHM after first failure

        // Plugin set = user-owned dofek.toml entries + managed plugins.toml.
        // The collector owns the merge so it can watch plugins.toml and
        // hot-reload when the user installs / removes / toggles a plugin via
        // the GUI or `dofek-tui plugins ...`.
        let plugin_store = PluginStore::open().ok();
        let plugins_toml_path = plugin_store
            .as_ref()
            .map(|s| s.plugins_toml().to_path_buf());
        let merge_plugins = |store: Option<&PluginStore>| -> Vec<PluginConfig> {
            let mut all = config.plugins.clone();
            if let Some(s) = store {
                all.extend(s.load_plugin_configs());
            }
            all
        };
        let mut plugin_manager = PluginManager::new(&merge_plugins(plugin_store.as_ref()));
        let mut plugins_toml_mtime = read_mtime(plugins_toml_path.as_deref());

        let hostname = System::host_name().unwrap_or_default();

        // sysinfo::System persists across polls for CPU% delta computation
        let mut system = System::new();

        // Linux: read CPU package temperature from /sys/class/hwmon via sysinfo.
        // Components is platform-specific in sysinfo 0.33 — only useful on Linux/macOS.
        #[cfg(target_os = "linux")]
        let mut components = sysinfo::Components::new_with_refreshed_list();

        #[cfg(target_os = "linux")]
        let mut rapl = rapl::RaplTracker::default();

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            // Hot-reload plugins if the managed plugins.toml has been touched
            // since the last tick. Cheap (one stat call) so it runs every
            // poll. The replace path sends shutdown to old children, kills
            // them after 200 ms, and respawns from the fresh config — that's
            // visible as one missed snapshot, no restart needed.
            let now_mtime = read_mtime(plugins_toml_path.as_deref());
            if now_mtime != plugins_toml_mtime {
                plugins_toml_mtime = now_mtime;
                let new_set = merge_plugins(plugin_store.as_ref());
                log::info!(
                    "plugins.toml changed, reloading plugin manager ({} plugin(s))",
                    new_set.len()
                );
                plugin_manager.replace(&new_set);
            }

            // Refresh sysinfo data
            system.refresh_cpu_all();
            system.refresh_memory();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            // CPU and memory from sysinfo (always available)
            let mut cpu = sysinfo_source::extract_cpu(&system);
            let memory = sysinfo_source::extract_memory(&system);

            // Linux: enrich CPU temperature from hwmon (Windows uses LHM below).
            #[cfg(target_os = "linux")]
            {
                components.refresh(true);
                if cpu.temperature.is_none() {
                    cpu.temperature = sysinfo_source::pick_cpu_temp(&components);
                }
                if cpu.power.is_none()
                    && let Some(w) = rapl.read_watts()
                {
                    cpu.power = Some(w);
                }
            }

            // GPU: try NVML first
            let nvml_snap = nvml.query();
            let nvml_gpus: Vec<GpuSensors> = nvml_snap.devices.iter().map(|dev| GpuSensors {
                name: dev.name.clone(),
                utilization: dev.utilization,
                vram_used_mb: dev.vram_used_mb,
                vram_total_mb: dev.vram_total_mb,
                temperature: dev.temperature,
                power_watts: dev.power_watts,
            }).collect();

            // Always try LHM for supplemental data (CPU temp/power, GPU fallback)
            let mut lhm_connected_now = false;
            let mut gpu_sensors: Vec<GpuSensors> = nvml_gpus;

            if !lhm_failed {
                match lhm::fetch_lhm_data(&config.lhm.url) {
                    Ok(root) => {
                        lhm_connected_now = true;

                        // Enrich CPU with temp/power from LHM (sysinfo can't provide these on Windows)
                        if let Some(lhm_cpu) = lhm::extract_cpu(&root) {
                            if cpu.temperature.is_none() {
                                cpu.temperature = lhm_cpu.temperature;
                            }
                            if cpu.power.is_none() {
                                cpu.power = lhm_cpu.power;
                            }
                        }

                        // GPU: use LHM as fallback if NVML returned nothing
                        if gpu_sensors.is_empty() {
                            gpu_sensors = lhm::extract_gpus(&root);
                        }
                    }
                    Err(_) => {
                        lhm_failed = true;
                        log::info!("LHM not available at {}, supplemental sensors disabled", config.lhm.url);
                    }
                }
            }

            let network = network::query_network_stats(&mut net_tracker);
            let disk = disk::query_disk_stats(&mut disk_tracker);

            // Processes from sysinfo (includes CPU%)
            let mut processes = sysinfo_source::enumerate_processes(
                &system,
                &nvml_snap.per_process_vram,
            );

            // Classify AI workloads and process categories
            let gpu_util = gpu_sensors.iter().map(|g| g.utilization).fold(0.0f32, f32::max);
            for proc in &mut processes {
                let prev = prev_vram.get(&proc.pid).copied();
                ai_detect::classify_process(proc, &config.ai, &config.categories, gpu_util, prev);
            }

            // Track VRAM for delta detection
            prev_vram.clear();
            for proc in &processes {
                if let Some(vram) = proc.vram_bytes {
                    prev_vram.insert(proc.pid, vram);
                }
            }

            // Publish the process context and collect plugin status. `tick` is
            // non-blocking: each plugin runs on its own supervisor thread, so a
            // wedged plugin no longer stalls this loop (and with it every
            // metric in both UIs).
            //
            // The context itself is ~500 `String` clones, so it is built only
            // when something is actually going to read it — it used to be built
            // on every tick of every install, plugins or not.
            let plugin_statuses = if plugin_manager.has_plugins() {
                let proc_context: Vec<ProcessContext> = processes
                    .iter()
                    .map(|p| ProcessContext {
                        pid: p.pid,
                        name: p.name.clone(),
                        vram_bytes: p.vram_bytes,
                    })
                    .collect();
                plugin_manager.tick(proc_context)
            } else {
                Vec::new()
            };

            // Apply plugin process annotations
            for status in &plugin_statuses {
                if let Some(ref response) = status.response {
                    for ann in &response.process_annotations {
                        if let Some(proc) = processes.iter_mut().find(|p| p.pid == ann.pid) {
                            if let Some(ref label) = ann.label {
                                proc.plugin_label = Some(label.clone());
                            }
                            if let Some(ref cat) = ann.category {
                                match cat.as_str() {
                                    "ai" => proc.category = process::ProcessCategory::Ai,
                                    "dev" => proc.category = process::ProcessCategory::Dev,
                                    "watch" => proc.category = process::ProcessCategory::Watch,
                                    _ => {}
                                }
                            }
                            if let Some(ref state) = ann.ai_state {
                                match state.as_str() {
                                    "idle" => proc.ai_state = process::AiState::Idle,
                                    "loading" => proc.ai_state = process::AiState::Loading,
                                    "inferring" => proc.ai_state = process::AiState::Inferring,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            let snapshot = DataSnapshot {
                cpu,
                memory,
                gpus: gpu_sensors,
                network,
                disk,
                processes,
                nvml_available: nvml.is_available(),
                lhm_connected: lhm_connected_now,
                hostname: hostname.clone(),
                timestamp: Instant::now(),
                plugin_statuses,
            };

            if tx.send(snapshot).is_err() {
                break; // Main thread dropped
            }

            // `recv_timeout` rather than `sleep` so a shutdown doesn't have to
            // wait out a full refresh interval — which the user can set as high
            // as they like.
            match wake_rx.recv_timeout(Duration::from_millis(refresh_ms.load(Ordering::Relaxed))) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                // Either an explicit wake or the handle was dropped: stop.
                _ => break,
            }
        }

        // Explicit rather than relying on drop order, because this is the step
        // that stops every plugin child process.
        plugin_manager.shutdown();
    });

    (
        rx,
        CollectorHandle {
            stop,
            wake: Some(wake),
            join: Some(join),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginState;
    use crate::plugin::protocol::{Panel, PanelEntry, PollResponse};

    fn snapshot_with_plugin() -> DataSnapshot {
        DataSnapshot {
            plugin_statuses: vec![PluginStatus {
                name: "ollama".into(),
                display_name: "Ollama".into(),
                state: PluginState::Unhealthy,
                response: Some(PollResponse {
                    panels: vec![Panel {
                        id: "models".into(),
                        label: "Models".into(),
                        content: vec![PanelEntry {
                            key: "loaded".into(),
                            value: "llama3:8b".into(),
                            style: "accent".into(),
                        }],
                    }],
                    ..Default::default()
                }),
            }],
            processes: vec![process::ProcessInfo {
                pid: 42,
                name: "ollama".into(),
                cpu_percent: 1.0,
                memory_bytes: 0,
                vram_bytes: None,
                is_ai_workload: true,
                ai_state: process::AiState::Inferring,
                category: process::ProcessCategory::Ai,
                plugin_label: Some("llama3:8b".into()),
            }],
            ..Default::default()
        }
    }

    /// `plugin_statuses` carried `#[serde(skip)]` until v1.7, so the GUI dock
    /// could only ever be static markup. Dropping the attribute is the whole
    /// premise of PR 5 — this is the guard against it coming back.
    #[test]
    fn plugin_statuses_reach_the_wire() {
        let json = serde_json::to_value(snapshot_with_plugin()).expect("snapshot serializes");
        let statuses = json["plugin_statuses"]
            .as_array()
            .expect("plugin_statuses must be serialized, not skipped");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0]["display_name"], "Ollama");
        assert_eq!(
            statuses[0]["response"]["panels"][0]["content"][0]["value"],
            "llama3:8b",
            "panel content must survive serialization — it is what the dock renders"
        );
    }

    /// The GUI keys its dot colours off these strings, so the wire form is
    /// part of the contract, not an implementation detail of the enum.
    #[test]
    fn plugin_state_serializes_lowercase() {
        let json = serde_json::to_value(snapshot_with_plugin()).unwrap();
        assert_eq!(json["plugin_statuses"][0]["state"], "unhealthy");

        for (state, expected) in [
            (PluginState::Starting, "starting"),
            (PluginState::Healthy, "healthy"),
            (PluginState::Unhealthy, "unhealthy"),
            (PluginState::Crashed, "crashed"),
        ] {
            assert_eq!(serde_json::to_value(state).unwrap(), expected);
            // The Display impl and the wire form must not drift apart.
            assert_eq!(state.to_string(), expected);
        }
    }

    /// `plugin_label` was serialized all along but never rendered. Now that
    /// both UIs draw it, it needs to survive the trip.
    #[test]
    fn plugin_label_reaches_the_wire() {
        let json = serde_json::to_value(snapshot_with_plugin()).unwrap();
        assert_eq!(json["processes"][0]["plugin_label"], "llama3:8b");
    }

    /// `skip_serializing_if` keeps the common case off the wire — ~500
    /// processes a second, almost none of them labelled.
    #[test]
    fn unlabelled_processes_carry_no_label_key() {
        let mut snap = snapshot_with_plugin();
        snap.processes[0].plugin_label = None;
        let json = serde_json::to_value(snap).unwrap();
        assert!(json["processes"][0].get("plugin_label").is_none());
    }
}
