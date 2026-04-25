//! dofek — shared data collection library.
//!
//! This module exposes the data collection layer so it can be used by
//! both the TUI binary and the Tauri GUI app.

pub mod config;
pub mod data;
pub mod plugin;
pub mod settings;
pub mod telemetry;

/// Returns a human-readable OS version string, e.g. "Windows 11" or "Ubuntu 24.04 LTS".
///
/// On Windows, uses RtlGetVersion (ntdll) which bypasses compatibility shims.
/// On Linux, parses /etc/os-release and returns PRETTY_NAME.
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

#[cfg(not(any(windows, target_os = "linux")))]
pub fn os_version_string() -> String {
    std::env::consts::OS.to_string()
}
