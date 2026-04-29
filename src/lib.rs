//! Dofek — shared data collection library.
//!
//! This module exposes the data collection layer so it can be used by
//! both the TUI binary and the Tauri GUI app.

pub mod config;
pub mod data;
pub mod plugin;
pub mod settings;
pub mod telemetry;
pub mod update;

/// Returns a human-readable OS version string, e.g. "Windows 11" or "Ubuntu 24.04 LTS".
///
/// On Windows, uses RtlGetVersion (ntdll) which bypasses compatibility shims.
/// On Linux, parses /etc/os-release and returns PRETTY_NAME.
/// On macOS, shells out to `sw_vers` and maps the major version to a codename.
#[cfg(windows)]
pub fn os_version_string() -> String {
    use windows::Wdk::System::SystemServices::RtlGetVersion;
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };

    let status = unsafe { RtlGetVersion(&mut info) };
    if status.is_err() {
        return "Windows".into();
    }

    let build = info.dwBuildNumber;
    if build >= 22000 { "Windows 11".into() } else { "Windows 10".into() }
}

#[cfg(target_os = "linux")]
pub fn os_version_string() -> String {
    let content = match std::fs::read_to_string("/etc/os-release") {
        Ok(s) => s,
        Err(_) => return "Linux".into(),
    };
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            return rest.trim().trim_matches('"').to_string();
        }
    }
    "Linux".into()
}

#[cfg(target_os = "macos")]
pub fn os_version_string() -> String {
    use std::process::Command;

    let run = |arg: &str| -> Option<String> {
        Command::new("sw_vers")
            .arg(arg)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let product = run("-productName").unwrap_or_else(|| "macOS".to_string());
    let version = run("-productVersion").unwrap_or_default();

    let codename = version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .and_then(|major| match major {
            15 => Some("Sequoia"),
            14 => Some("Sonoma"),
            13 => Some("Ventura"),
            12 => Some("Monterey"),
            11 => Some("Big Sur"),
            _ => None,
        });

    match (version.is_empty(), codename) {
        (false, Some(c)) => format!("{product} {version} ({c})"),
        (false, None) => format!("{product} {version}"),
        (true, _) => product,
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn os_version_string() -> String {
    std::env::consts::OS.to_string()
}

/// Labels shown when no discrete GPU is detected. Apple Silicon Macs aren't
/// "no GPU" — the GPU is integrated and shares system memory — so we surface
/// the chip name and point the user at the MEM panel instead.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GpuEmptyState {
    pub title: String,
    pub body: String,
}

/// Returns the platform-appropriate empty-state labels. Cached on first call.
pub fn gpu_empty_state() -> &'static GpuEmptyState {
    static CACHED: std::sync::OnceLock<GpuEmptyState> = std::sync::OnceLock::new();
    CACHED.get_or_init(compute_gpu_empty_state)
}

#[cfg(target_os = "macos")]
fn compute_gpu_empty_state() -> GpuEmptyState {
    let chip = std::process::Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Apple Silicon".to_string());
    GpuEmptyState {
        title: format!("{chip} GPU"),
        body: "integrated · unified memory — see MEM panel".to_string(),
    }
}

#[cfg(not(target_os = "macos"))]
fn compute_gpu_empty_state() -> GpuEmptyState {
    GpuEmptyState {
        title: "No GPU".to_string(),
        body: "No GPU detected".to_string(),
    }
}
