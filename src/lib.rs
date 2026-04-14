//! dofek — shared data collection library.
//!
//! This module exposes the data collection layer so it can be used by
//! both the TUI binary and the Tauri GUI app.

pub mod config;
pub mod data;
pub mod plugin;
pub mod settings;
pub mod telemetry;

/// Returns a human-readable Windows version string, e.g. "Windows 11".
/// Uses RtlGetVersion (ntdll) which bypasses compatibility shims and
/// always returns the real OS version, unlike GetVersionExW.
#[cfg(windows)]
pub fn windows_version_string() -> String {
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

#[cfg(not(windows))]
pub fn windows_version_string() -> String {
    std::env::var("OS").unwrap_or_else(|_| "Unknown".into())
}
