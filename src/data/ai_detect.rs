use crate::config::AiConfig;
use crate::data::process::{AiState, ProcessInfo};

/// Classify a process as an AI workload and determine its state.
pub fn classify_ai_workload(
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
