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

/// Pick the most representative CPU temperature from sysinfo Components on Linux.
///
/// Tries package/die-level sensors first (best signal), then falls back to averaging
/// per-core sensors. Common labels by vendor:
///   - Intel coretemp: "Package id 0", "Core 0".."Core N"
///   - AMD k10temp:    "Tctl", "Tdie"
///   - ARM/embedded:   "cpu_thermal", "cpu-thermal 0"
#[cfg(target_os = "linux")]
pub fn pick_cpu_temp(components: &sysinfo::Components) -> Option<f32> {
    pick_temp_from(components.iter().map(|c| (c.label(), c.temperature())))
}

/// The label-priority logic behind [`pick_cpu_temp`], over plain
/// `(label, temperature)` pairs.
///
/// Split out from the `sysinfo::Components` call so it is testable — and
/// tested on every OS, not just the one platform that compiles the caller.
/// `Components` has no public constructor that lets a test seed labels.
pub fn pick_temp_from<'a, I>(components: I) -> Option<f32>
where
    I: IntoIterator<Item = (&'a str, Option<f32>)> + Clone,
{
    // Preferred package-level labels in priority order.
    const PACKAGE_LABELS: &[&str] = &["Package id 0", "Tctl", "Tdie", "cpu_thermal", "cpu-thermal"];

    for pref in PACKAGE_LABELS {
        for (label, temp) in components.clone() {
            if label.contains(pref)
                && let Some(t) = temp
            {
                return Some(t);
            }
        }
    }

    // Fallback: average per-core readings if any are present.
    let cores: Vec<f32> = components
        .into_iter()
        .filter(|(label, _)| label.starts_with("Core "))
        .filter_map(|(_, temp)| temp)
        .collect();
    if cores.is_empty() {
        None
    } else {
        Some(cores.iter().sum::<f32>() / cores.len() as f32)
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
                plugin_label: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Package-level sensors are preferred, in the declared priority order.
    #[test]
    fn prefers_package_labels_in_order() {
        // Intel coretemp naming.
        let intel = [("Package id 0", Some(55.0)), ("Core 0", Some(70.0))];
        assert_eq!(pick_temp_from(intel.iter().copied()), Some(55.0));

        // AMD k10temp: Tctl outranks Tdie.
        let amd = [("Tdie", Some(48.0)), ("Tctl", Some(60.0))];
        assert_eq!(pick_temp_from(amd.iter().copied()), Some(60.0));

        // ARM / embedded.
        let arm = [("cpu_thermal", Some(41.5))];
        assert_eq!(pick_temp_from(arm.iter().copied()), Some(41.5));

        // Labels match as substrings, the way sysinfo reports them.
        let suffixed = [("k10temp Tctl temp1", Some(62.0))];
        assert_eq!(pick_temp_from(suffixed.iter().copied()), Some(62.0));
    }

    /// A preferred label with no reading must not shadow the fallback.
    #[test]
    fn skips_preferred_labels_that_report_no_temperature() {
        let c = [("Package id 0", None), ("Core 0", Some(70.0)), ("Core 1", Some(80.0))];
        assert_eq!(pick_temp_from(c.iter().copied()), Some(75.0));
    }

    #[test]
    fn averages_per_core_readings_as_a_fallback() {
        let c = [("Core 0", Some(60.0)), ("Core 1", Some(70.0)), ("Core 2", Some(80.0))];
        assert_eq!(pick_temp_from(c.iter().copied()), Some(70.0));

        // Cores that report nothing are excluded from the average, not counted
        // as zero.
        let partial = [("Core 0", Some(60.0)), ("Core 1", None), ("Core 2", Some(80.0))];
        assert_eq!(pick_temp_from(partial.iter().copied()), Some(70.0));
    }

    #[test]
    fn returns_none_when_nothing_usable_is_present() {
        let empty: [(&str, Option<f32>); 0] = [];
        assert_eq!(pick_temp_from(empty.iter().copied()), None);

        // Non-CPU sensors only.
        let unrelated = [("nvme Composite", Some(38.0)), ("acpitz", Some(45.0))];
        assert_eq!(pick_temp_from(unrelated.iter().copied()), None);

        // "Core " needs the trailing space — "Corsair" must not match.
        let lookalike = [("Corsair H100i", Some(30.0))];
        assert_eq!(pick_temp_from(lookalike.iter().copied()), None);

        // Everything present but unreadable.
        let no_readings = [("Package id 0", None), ("Core 0", None)];
        assert_eq!(pick_temp_from(no_readings.iter().copied()), None);
    }
}
