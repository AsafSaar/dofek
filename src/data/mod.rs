pub mod ai_detect;
pub mod disk;
pub mod gpu;
pub mod lhm;
pub mod network;
pub mod process;
#[cfg(target_os = "linux")]
pub mod rapl;
pub mod sysinfo_source;

use crate::config::Config;
use crate::plugin::{PluginManager, PluginStatus};
use crate::plugin::protocol::ProcessContext;
use disk::{DiskStats, DiskTracker};
use lhm::{CpuSensors, GpuSensors, MemorySensors};
use network::{NetworkStats, NetworkTracker};
use process::ProcessInfo;
use gpu::NvmlState;
use std::collections::HashMap;
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
    #[serde(skip)]
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

/// Spawn the data collector thread. Returns a receiver for snapshots.
pub fn spawn_collector(config: Config) -> mpsc::Receiver<DataSnapshot> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut net_tracker = NetworkTracker::default();
        let mut disk_tracker = DiskTracker::default();
        let nvml = NvmlState::init();
        let mut prev_vram: HashMap<u32, u64> = HashMap::new();
        let mut lhm_failed = false; // stop retrying LHM after first failure
        let mut plugin_manager = PluginManager::new(&config.plugins);
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
                if cpu.power.is_none() {
                    if let Some(w) = rapl.read_watts() {
                        cpu.power = Some(w);
                    }
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

            // Poll plugins with process context
            let proc_context: Vec<ProcessContext> = processes
                .iter()
                .map(|p| ProcessContext {
                    pid: p.pid,
                    name: p.name.clone(),
                    vram_bytes: p.vram_bytes,
                })
                .collect();
            let plugin_statuses = plugin_manager.poll_all(&proc_context);

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
                return; // Main thread dropped, exit
            }

            thread::sleep(Duration::from_millis(config.general.refresh_ms));
        }
    });

    rx
}
