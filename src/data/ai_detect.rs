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
        || categories_config.watch_lower.iter().any(|w| name_lower.contains(w.as_str()));
    if is_watch {
        proc.category = ProcessCategory::Watch;
    } else if proc.is_ai_workload {
        proc.category = ProcessCategory::Ai;
    } else {
        let is_dev = categories_config.dev_lower.iter().any(|dev| name_lower.contains(dev.as_str()));
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
    let name_matches = config.known_ai_lower.iter().any(|known| name_lower.contains(known.as_str()));

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
    } else {
        proc.ai_state = AiState::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    /// Config mirroring what `Config::load` produces: the `*_lower` fields are
    /// pre-lowercased at load time and are the ones classification reads.
    fn ai_config(known: &[&str], threshold_gb: f64) -> AiConfig {
        AiConfig {
            vram_threshold_gb: threshold_gb,
            known_ai_processes: known.iter().map(|s| s.to_string()).collect(),
            known_ai_lower: known.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    fn categories(dev: &[&str], watch: &[&str], watch_pids: &[u32]) -> CategoriesConfig {
        CategoriesConfig {
            dev_processes: dev.iter().map(|s| s.to_string()).collect(),
            watch_processes: watch.iter().map(|s| s.to_string()).collect(),
            watch_pids: watch_pids.to_vec(),
            dev_lower: dev.iter().map(|s| s.to_lowercase()).collect(),
            watch_lower: watch.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    fn process(pid: u32, name: &str, vram: Option<u64>) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cpu_percent: 0.0,
            memory_bytes: 0,
            vram_bytes: vram,
            is_ai_workload: false,
            ai_state: AiState::None,
            category: ProcessCategory::None,
            plugin_label: None,
        }
    }

    /// One row of the classification table below.
    struct Case {
        name: &'static str,
        vram: Option<u64>,
        gpu_util: f32,
        prev_vram: Option<u64>,
        want_ai: bool,
        want_state: AiState,
    }

    /// Terse constructor so the table stays readable as a table.
    fn case(
        name: &'static str,
        vram: Option<u64>,
        gpu_util: f32,
        prev_vram: Option<u64>,
        want_ai: bool,
        want_state: AiState,
    ) -> Case {
        Case { name, vram, gpu_util, prev_vram, want_ai, want_state }
    }

    /// Every branch of the AI-workload / state decision, as a table.
    #[test]
    fn ai_workload_and_state_branches() {
        let cfg = ai_config(&["ollama", "python"], 1.0);

        // name, vram, gpu_util, prev_vram -> (is_ai, state)
        let cases = [
            // Known-name match with no VRAM at all: AI, but idle.
            case("ollama", None, 0.0, None, true, AiState::Idle),
            // Case-insensitive, and substring rather than exact match.
            case("Ollama.exe", None, 0.0, None, true, AiState::Idle),
            case("python3.12", None, 0.0, None, true, AiState::Idle),
            // Unknown name, VRAM under the threshold: not an AI workload.
            case("chrome", Some(GB / 2), 90.0, None, false, AiState::None),
            // Unknown name, VRAM over the threshold: flagged on VRAM alone.
            case("mystery", Some(2 * GB), 0.0, None, true, AiState::Idle),
            // Over threshold + busy GPU: inferring.
            case("mystery", Some(2 * GB), 21.0, None, true, AiState::Inferring),
            // Busy GPU but under threshold: known-name only, so still idle.
            case("ollama", Some(GB / 2), 90.0, None, true, AiState::Idle),
            // Exactly at the threshold counts as over (>=).
            case("mystery", Some(GB), 21.0, None, true, AiState::Inferring),
            // GPU utilization exactly at the boundary is not "inferring" (>).
            case("mystery", Some(2 * GB), 20.0, None, true, AiState::Idle),
            // The `_server` heuristic: any nonzero VRAM is enough.
            case("llama_server", Some(1), 0.0, None, true, AiState::Idle),
            // ...but zero VRAM is not, and the name isn't otherwise known.
            case("llama_server", Some(0), 0.0, None, false, AiState::None),
            case("llama_server", None, 0.0, None, false, AiState::None),
            // Loading beats everything: VRAM climbing by >200 MB since last tick.
            case("ollama", Some(GB), 90.0, Some(0), true, AiState::Loading),
            case("mystery", Some(2 * GB), 90.0, Some(GB), true, AiState::Loading),
            // A small climb is not loading.
            case("mystery", Some(2 * GB), 90.0, Some(2 * GB - 1024), true, AiState::Inferring),
            // VRAM shrinking is not loading either.
            case("mystery", Some(GB), 0.0, Some(4 * GB), true, AiState::Idle),
            // Non-AI processes never get a state, whatever the deltas say.
            case("chrome", Some(0), 90.0, Some(0), false, AiState::None),
        ];

        for c in cases {
            let Case { name, vram, gpu_util, prev_vram, want_ai, want_state } = c;
            let mut p = process(1, name, vram);
            classify_process(&mut p, &cfg, &CategoriesConfig::default(), gpu_util, prev_vram);
            let ctx = format!("{name} vram={vram:?} util={gpu_util} prev={prev_vram:?}");
            assert_eq!(p.is_ai_workload, want_ai, "{ctx}: is_ai_workload");
            assert_eq!(p.ai_state, want_state, "{ctx}: ai_state");
        }
    }

    /// Category priority is Watch > Ai > Dev > None.
    #[test]
    fn category_priority() {
        let ai = ai_config(&["ollama"], 1.0);
        let cats = categories(&["code", "cargo"], &["ollama", "keepme"], &[4242]);

        // Watch wins over Ai even though the name is also a known AI process.
        let mut p = process(1, "ollama", Some(8 * GB));
        classify_process(&mut p, &ai, &cats, 50.0, None);
        assert_eq!(p.category, ProcessCategory::Watch);
        assert!(p.is_ai_workload, "watch pinning must not clear the AI flag");

        // A pinned PID is watched regardless of name.
        let mut p = process(4242, "chrome", None);
        classify_process(&mut p, &ai, &cats, 0.0, None);
        assert_eq!(p.category, ProcessCategory::Watch);

        // Ai wins over Dev: "code" is a dev name, but the VRAM flags it as AI.
        let mut p = process(2, "code", Some(4 * GB));
        classify_process(&mut p, &ai, &cats, 0.0, None);
        assert_eq!(p.category, ProcessCategory::Ai);

        // Dev with no AI signal.
        let mut p = process(3, "code", None);
        classify_process(&mut p, &ai, &cats, 0.0, None);
        assert_eq!(p.category, ProcessCategory::Dev);

        // Nothing matches.
        let mut p = process(5, "finder", None);
        classify_process(&mut p, &ai, &cats, 0.0, None);
        assert_eq!(p.category, ProcessCategory::None);
    }

    #[test]
    fn category_matching_is_case_insensitive_and_substring() {
        let ai = ai_config(&[], 1.0);
        let cats = categories(&["code"], &[], &[]);

        for name in ["Code", "VSCode.exe", "CODE"] {
            let mut p = process(1, name, None);
            classify_process(&mut p, &ai, &cats, 0.0, None);
            assert_eq!(p.category, ProcessCategory::Dev, "{name}");
        }
    }

    /// Classification must be idempotent — the collector reuses `ProcessInfo`
    /// values that already carry a category from a previous tick.
    #[test]
    fn reclassifying_clears_a_stale_category() {
        let ai = ai_config(&["ollama"], 1.0);
        let cats = CategoriesConfig::default();

        let mut p = process(1, "ollama", Some(8 * GB));
        classify_process(&mut p, &ai, &cats, 90.0, None);
        assert_eq!(p.category, ProcessCategory::Ai);
        assert_eq!(p.ai_state, AiState::Inferring);

        // The same struct, now renamed to something unremarkable with no VRAM.
        p.name = "finder".to_string();
        p.vram_bytes = None;
        classify_process(&mut p, &ai, &cats, 0.0, None);
        assert_eq!(p.category, ProcessCategory::None);
        assert_eq!(p.ai_state, AiState::None);
        assert!(!p.is_ai_workload);
    }

    #[test]
    fn empty_config_classifies_nothing() {
        let ai = ai_config(&[], 1.0);
        let cats = categories(&[], &[], &[]);
        let mut p = process(1, "anything", None);
        classify_process(&mut p, &ai, &cats, 100.0, None);
        assert!(!p.is_ai_workload);
        assert_eq!(p.category, ProcessCategory::None);
        assert_eq!(p.ai_state, AiState::None);
    }
}
