//! dofek — shared data collection library.
//!
//! This module exposes the data collection layer so it can be used by
//! both the TUI binary and the Tauri GUI app.

pub mod config;
pub mod data;
pub mod plugin;
pub mod settings;
pub mod telemetry;

/// Returns a human-readable Windows version string, e.g. "Windows 11 (26200)".
/// Falls back to the `OS` env var if the API call fails.
#[cfg(windows)]
pub fn windows_version_string() -> String {
    use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOW};

    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };

    let ok = unsafe { GetVersionExW(&mut info).is_ok() };
    if !ok {
        return std::env::var("OS").unwrap_or_else(|_| "Windows".into());
    }

    let build = info.dwBuildNumber;
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    format!("{name} ({build})")
}

#[cfg(not(windows))]
pub fn windows_version_string() -> String {
    std::env::var("OS").unwrap_or_else(|_| "Unknown".into())
}
