#[cfg(windows)]
use windows::Win32::System::ProcessStatus::{
    EnumProcesses, K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;

use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub vram_bytes: Option<u64>,
    pub is_ai_workload: bool,
    pub ai_state: AiState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiState {
    None,
    Idle,
    Loading,
    Inferring,
}

impl std::fmt::Display for AiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiState::None => write!(f, ""),
            AiState::Idle => write!(f, "idle"),
            AiState::Loading => write!(f, "loading"),
            AiState::Inferring => write!(f, "inferring"),
        }
    }
}

/// Snapshot of per-process CPU times for computing CPU% between samples.
#[derive(Default)]
pub struct ProcessCpuTracker {
    prev_times: HashMap<u32, (u64, Instant)>, // pid -> (kernel+user ticks, when)
}

/// Enumerate all processes and their memory usage.
#[cfg(windows)]
pub fn enumerate_processes(
    _cpu_tracker: &mut ProcessCpuTracker,
    per_process_vram: &HashMap<u32, u64>,
) -> Vec<ProcessInfo> {
    let mut pids = vec![0u32; 2048];
    let mut bytes_returned = 0u32;

    unsafe {
        let ok = EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut bytes_returned,
        );
        if ok.is_err() {
            return Vec::new();
        }
    }

    let num_pids = bytes_returned as usize / std::mem::size_of::<u32>();
    let mut processes = Vec::new();

    for &pid in &pids[..num_pids] {
        if pid == 0 {
            continue;
        }

        let handle = unsafe {
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
        };
        let handle = match handle {
            Ok(h) => h,
            Err(_) => continue, // Access denied for system processes
        };

        // Get process name
        let name = get_process_name(handle, pid);

        // Get memory info
        let mut mem_counters = PROCESS_MEMORY_COUNTERS::default();
        let mem_size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let memory_bytes = unsafe {
            if K32GetProcessMemoryInfo(handle, &mut mem_counters, mem_size).as_bool() {
                mem_counters.WorkingSetSize
            } else {
                0
            }
        };

        let vram_bytes = per_process_vram.get(&pid).copied();

        unsafe { let _ = CloseHandle(handle); }

        if !name.is_empty() {
            processes.push(ProcessInfo {
                pid,
                name,
                cpu_percent: 0.0, // TODO: compute from kernel/user times delta
                memory_bytes: memory_bytes as u64,
                vram_bytes,
                is_ai_workload: false,
                ai_state: AiState::None,
            });
        }
    }

    // Sort by memory descending as a starting default
    processes.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
    processes
}

#[cfg(windows)]
fn get_process_name(handle: windows::Win32::Foundation::HANDLE, _pid: u32) -> String {
    use windows::Win32::System::ProcessStatus::K32GetModuleBaseNameW;

    let mut name_buf = [0u16; 260];
    let len = unsafe {
        K32GetModuleBaseNameW(handle, None, &mut name_buf)
    };
    if len > 0 {
        String::from_utf16_lossy(&name_buf[..len as usize])
    } else {
        String::new()
    }
}

#[cfg(not(windows))]
pub fn enumerate_processes(
    _cpu_tracker: &mut ProcessCpuTracker,
    _per_process_vram: &HashMap<u32, u64>,
) -> Vec<ProcessInfo> {
    Vec::new()
}
