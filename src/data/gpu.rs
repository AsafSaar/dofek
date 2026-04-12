use std::collections::HashMap;
use nvml_wrapper::enums::device::UsedGpuMemory;

/// Wrapper around NVML for per-process VRAM queries.
/// Gracefully handles missing NVIDIA drivers.
pub struct NvmlState {
    available: bool,
}

/// Per-process VRAM data from NVML.
#[derive(Debug, Clone, Default)]
pub struct NvmlSnapshot {
    pub per_process_vram: HashMap<u32, u64>, // pid -> bytes
}

fn extract_gpu_mem(mem: UsedGpuMemory) -> u64 {
    match mem {
        UsedGpuMemory::Used(bytes) => bytes,
        UsedGpuMemory::Unavailable => 0,
    }
}

impl NvmlState {
    pub fn init() -> Self {
        match nvml_wrapper::Nvml::init() {
            Ok(_nvml) => {
                log::info!("NVML initialized successfully");
                Self { available: true }
            }
            Err(e) => {
                log::warn!("NVML not available: {e}. Per-process VRAM will be disabled.");
                Self { available: false }
            }
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Query per-process VRAM usage from NVML.
    pub fn query_per_process_vram(&self) -> NvmlSnapshot {
        if !self.available {
            return NvmlSnapshot::default();
        }

        // Re-init NVML for each query (simple approach for POC)
        let nvml = match nvml_wrapper::Nvml::init() {
            Ok(n) => n,
            Err(_) => return NvmlSnapshot::default(),
        };

        let mut per_process_vram = HashMap::new();

        let device_count = nvml.device_count().unwrap_or(0);
        for i in 0..device_count {
            let device = match nvml.device_by_index(i) {
                Ok(d) => d,
                Err(_) => continue,
            };

            if let Ok(procs) = device.running_compute_processes() {
                for p in procs {
                    let mem = extract_gpu_mem(p.used_gpu_memory);
                    *per_process_vram.entry(p.pid).or_insert(0) += mem;
                }
            }

            if let Ok(procs) = device.running_graphics_processes() {
                for p in procs {
                    let mem = extract_gpu_mem(p.used_gpu_memory);
                    *per_process_vram.entry(p.pid).or_insert(0) += mem;
                }
            }
        }

        NvmlSnapshot { per_process_vram }
    }
}
