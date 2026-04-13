#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum ProcessCategory {
    None,
    Ai,
    Dev,
    Watch,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub vram_bytes: Option<u64>,
    pub is_ai_workload: bool,
    pub ai_state: AiState,
    pub category: ProcessCategory,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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
