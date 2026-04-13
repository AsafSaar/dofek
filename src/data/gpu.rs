use std::collections::HashMap;
use nvml_wrapper::enums::device::UsedGpuMemory;

/// Wrapper around NVML for GPU queries.
/// Gracefully handles missing NVIDIA drivers.
pub struct NvmlState {
    available: bool,
}

/// Per-process VRAM data from NVML.
#[derive(Debug, Clone, Default)]
pub struct NvmlSnapshot {
    pub per_process_vram: HashMap<u32, u64>, // pid -> bytes
    pub device: Option<GpuDeviceInfo>,
}

/// Device-level GPU metrics from NVML.
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    pub name: String,
    pub utilization: f32,
    pub vram_used_mb: f32,
    pub vram_total_mb: f32,
    pub temperature: f32,
    pub power_watts: f32,
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
                log::warn!("NVML not available: {e}. GPU data will be disabled.");
                Self { available: false }
            }
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Query device-level GPU metrics and per-process VRAM usage.
    pub fn query(&self) -> NvmlSnapshot {
        if !self.available {
            return NvmlSnapshot::default();
        }

        // Re-init NVML for each query (simple approach for POC)
        let nvml = match nvml_wrapper::Nvml::init() {
            Ok(n) => n,
            Err(_) => return NvmlSnapshot::default(),
        };

        let mut per_process_vram = HashMap::new();
        let mut device_info = None;

        let device_count = nvml.device_count().unwrap_or(0);
        for i in 0..device_count {
            let device = match nvml.device_by_index(i) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Device-level metrics (use first GPU)
            if device_info.is_none() {
                let name = device.name().unwrap_or_else(|_| "NVIDIA GPU".to_string());

                let utilization = device.utilization_rates()
                    .map(|u| u.gpu as f32)
                    .unwrap_or(0.0);

                let (vram_used_mb, vram_total_mb) = device.memory_info()
                    .map(|m| (m.used as f32 / 1024.0 / 1024.0, m.total as f32 / 1024.0 / 1024.0))
                    .unwrap_or((0.0, 0.0));

                let temperature = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                    .map(|t| t as f32)
                    .unwrap_or(0.0);

                let power_watts = device.power_usage()
                    .map(|mw| mw as f32 / 1000.0)
                    .unwrap_or(0.0);

                device_info = Some(GpuDeviceInfo {
                    name,
                    utilization,
                    vram_used_mb,
                    vram_total_mb,
                    temperature,
                    power_watts,
                });
            }

            // Per-process VRAM
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

        NvmlSnapshot { per_process_vram, device: device_info }
    }
}
