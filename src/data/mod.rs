pub mod ai_detect;
pub mod gpu;
pub mod lhm;
pub mod network;
pub mod process;
pub mod sysinfo_source;

use crate::config::Config;
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
#[derive(Debug, Clone)]
pub struct DataSnapshot {
    pub cpu: CpuSensors,
    pub memory: MemorySensors,
    pub gpu: Option<GpuSensors>,
    pub network: NetworkStats,
    pub processes: Vec<ProcessInfo>,
    pub nvml_available: bool,
    pub lhm_connected: bool,
    pub timestamp: Instant,
}

impl Default for DataSnapshot {
    fn default() -> Self {
        Self {
            cpu: CpuSensors::default(),
            memory: MemorySensors::default(),
            gpu: None,
            network: NetworkStats::default(),
            processes: Vec::new(),
            nvml_available: false,
            lhm_connected: false,
            timestamp: Instant::now(),
        }
    }
}

/// Spawn the data collector thread. Returns a receiver for snapshots.
pub fn spawn_collector(config: Config) -> mpsc::Receiver<DataSnapshot> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut net_tracker = NetworkTracker::default();
        let nvml = NvmlState::init();
        let mut prev_vram: HashMap<u32, u64> = HashMap::new();
        let mut lhm_failed = false; // stop retrying LHM after first failure

        // sysinfo::System persists across polls for CPU% delta computation
        let mut system = System::new();

        loop {
            // Refresh sysinfo data
            system.refresh_cpu_all();
            system.refresh_memory();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            // CPU and memory from sysinfo (always available)
            let cpu = sysinfo_source::extract_cpu(&system);
            let memory = sysinfo_source::extract_memory(&system);

            // GPU: try NVML first, fall back to LHM (only if NVML unavailable)
            let nvml_snap = nvml.query();
            let gpu_sensors = if let Some(ref dev) = nvml_snap.device {
                Some(GpuSensors {
                    name: dev.name.clone(),
                    utilization: dev.utilization,
                    vram_used_mb: dev.vram_used_mb,
                    vram_total_mb: dev.vram_total_mb,
                    temperature: dev.temperature,
                    power_watts: dev.power_watts,
                })
            } else if !lhm_failed {
                // Fallback: try LHM for GPU data (e.g. AMD GPUs)
                match lhm::fetch_lhm_data(&config.lhm.url) {
                    Ok(root) => lhm::extract_gpu(&root),
                    Err(_) => {
                        lhm_failed = true;
                        log::info!("LHM not available, GPU panel disabled");
                        None
                    }
                }
            } else {
                None
            };

            let network = network::query_network_stats(&mut net_tracker);

            // Processes from sysinfo (includes CPU%)
            let mut processes = sysinfo_source::enumerate_processes(
                &system,
                &nvml_snap.per_process_vram,
            );

            // Classify AI workloads
            let gpu_util = gpu_sensors.as_ref().map(|g| g.utilization).unwrap_or(0.0);
            for proc in &mut processes {
                let prev = prev_vram.get(&proc.pid).copied();
                ai_detect::classify_ai_workload(proc, &config.ai, gpu_util, prev);
            }

            // Track VRAM for delta detection
            prev_vram.clear();
            for proc in &processes {
                if let Some(vram) = proc.vram_bytes {
                    prev_vram.insert(proc.pid, vram);
                }
            }

            let snapshot = DataSnapshot {
                cpu,
                memory,
                gpu: gpu_sensors,
                network,
                processes,
                nvml_available: nvml.is_available(),
                lhm_connected: true, // sysinfo always provides data
                timestamp: Instant::now(),
            };

            if tx.send(snapshot).is_err() {
                return; // Main thread dropped, exit
            }

            thread::sleep(Duration::from_millis(config.general.refresh_ms));
        }
    });

    rx
}
