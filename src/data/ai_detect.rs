use crate::config::{AiConfig, CategoriesConfig};
use crate::data::process::{AiState, ProcessCategory, ProcessInfo};

/// Classify a process as an AI workload, determine its state, and assign a category.
pub fn classify_process(
    proc: &mut ProcessInfo,
    ai_config: &AiConfig,
    categories_config: &CategoriesConfig,
    gpu_utilization: f32,
    prev_vram: Option<u64>,
) {
    classify_ai_workload(proc, ai_config, gpu_utilization, prev_vram);

    // Assign category. Priority: Watch > Ai > Dev > None
    let name_lower = proc.name.to_lowercase();
    let is_watch = categories_config.watch_pids.contains(&proc.pid)
        || categories_config.watch_processes.iter().any(|w| {
            name_lower.contains(&w.to_lowercase())
        });
    if is_watch {
        proc.category = ProcessCategory::Watch;
    } else if proc.is_ai_workload {
        proc.category = ProcessCategory::Ai;
    } else {
        let is_dev = categories_config.dev_processes.iter().any(|dev| {
            name_lower.contains(&dev.to_lowercase())
        });
        if is_dev {
            proc.category = ProcessCategory::Dev;
        } else {
            proc.category = ProcessCategory::None;
        }
    }
}

fn classify_ai_workload(
    proc: &mut ProcessInfo,
    config: &AiConfig,
    gpu_utilization: f32,
    prev_vram: Option<u64>,
) {
    let name_lower = proc.name.to_lowercase();
    let name_matches = config.known_ai_processes.iter().any(|known| {
        name_lower.contains(&known.to_lowercase())
    });

    let vram_gb = proc.vram_bytes.map(|v| v as f64 / (1024.0 * 1024.0 * 1024.0)).unwrap_or(0.0);
    let over_threshold = vram_gb >= config.vram_threshold_gb;

    // Process name ends with _server and uses any VRAM
    let is_server_with_vram = name_lower.ends_with("_server") && proc.vram_bytes.unwrap_or(0) > 0;

    proc.is_ai_workload = name_matches || over_threshold || is_server_with_vram;

    if !proc.is_ai_workload {
        proc.ai_state = AiState::None;
        return;
    }

    let vram_bytes = proc.vram_bytes.unwrap_or(0);

    // Determine state
    if let Some(prev) = prev_vram {
        let delta = vram_bytes as i64 - prev as i64;
        // Loading: VRAM increasing rapidly (>200MB delta)
        if delta > 200 * 1024 * 1024 {
            proc.ai_state = AiState::Loading;
            return;
        }
    }

    if over_threshold && gpu_utilization > 20.0 {
        proc.ai_state = AiState::Inferring;
    } else if vram_bytes < 500 * 1024 * 1024 {
        proc.ai_state = AiState::Idle;
    } else {
        proc.ai_state = AiState::Idle;
    }
}
