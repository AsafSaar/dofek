pub mod ai_detect;
pub mod gpu;
pub mod lhm;
pub mod network;
pub mod process;

use crate::config::Config;
use lhm::{CpuSensors, GpuSensors, MemorySensors};
use network::{NetworkStats, NetworkTracker};
use process::{ProcessCpuTracker, ProcessInfo};
use gpu::NvmlState;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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
        let mut cpu_tracker = ProcessCpuTracker::default();
        let nvml = NvmlState::init();
        let mut prev_vram: HashMap<u32, u64> = HashMap::new();

        loop {
            let lhm_data = lhm::fetch_lhm_data(&config.lhm.url);

            let (cpu, memory, gpu_sensors, lhm_connected) = match &lhm_data {
                Ok(root) => {
                    let cpu = lhm::extract_cpu(root).unwrap_or_default();
                    let mem = lhm::extract_memory(root).unwrap_or_default();
                    let gpu = lhm::extract_gpu(root);
                    (cpu, mem, gpu, true)
                }
                Err(e) => {
                    log::debug!("LHM fetch failed: {e}");
                    (CpuSensors::default(), MemorySensors::default(), None, false)
                }
            };

            let nvml_snap = nvml.query_per_process_vram();
            let network = network::query_network_stats(&mut net_tracker);
            let mut processes = process::enumerate_processes(
                &mut cpu_tracker,
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
                lhm_connected,
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
