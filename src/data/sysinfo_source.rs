use sysinfo::System;
use std::collections::HashMap;

use crate::data::lhm::{CpuSensors, MemorySensors};
use crate::data::process::{ProcessInfo, AiState};

/// Query CPU sensors from sysinfo.
pub fn extract_cpu(system: &System) -> CpuSensors {
    let cpus = system.cpus();

    let name = cpus.first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();

    let per_core_load: Vec<f32> = cpus.iter()
        .map(|c| c.cpu_usage())
        .collect();

    let total_load = if per_core_load.is_empty() {
        0.0
    } else {
        per_core_load.iter().sum::<f32>() / per_core_load.len() as f32
    };

    CpuSensors {
        name,
        total_load,
        per_core_load,
        temperature: None, // sysinfo doesn't provide CPU temp on Windows without elevation
        power: None,
    }
}

/// Query memory sensors from sysinfo.
pub fn extract_memory(system: &System) -> MemorySensors {
    let total_bytes = system.total_memory();
    let used_bytes = system.used_memory();
    let total_gb = total_bytes as f32 / 1024.0 / 1024.0 / 1024.0;
    let used_gb = used_bytes as f32 / 1024.0 / 1024.0 / 1024.0;

    let used_percent = if total_bytes > 0 {
        used_bytes as f32 / total_bytes as f32 * 100.0
    } else {
        0.0
    };

    let total_swap = system.total_swap();
    let used_swap = system.used_swap();
    let swap_used_percent = if total_swap > 0 {
        used_swap as f32 / total_swap as f32 * 100.0
    } else {
        0.0
    };

    MemorySensors {
        used_percent,
        used_gb,
        total_gb,
        swap_used_percent,
    }
}

/// Enumerate processes from sysinfo, merging in NVML VRAM data.
pub fn enumerate_processes(
    system: &System,
    per_process_vram: &HashMap<u32, u64>,
) -> Vec<ProcessInfo> {
    system.processes().values()
        .filter_map(|proc| {
            let name = proc.name().to_string_lossy().to_string();
            if name.is_empty() {
                return None;
            }

            let pid = proc.pid().as_u32();
            let vram_bytes = per_process_vram.get(&pid).copied();

            Some(ProcessInfo {
                pid,
                name,
                cpu_percent: proc.cpu_usage(),
                memory_bytes: proc.memory(),
                vram_bytes,
                is_ai_workload: false,
                ai_state: AiState::None,
                category: crate::data::process::ProcessCategory::None,
            })
        })
        .collect()
}
