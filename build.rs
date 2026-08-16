//! Embeds a Windows VERSIONINFO resource into the binaries this crate builds.
//!
//! Why this exists: the SignPath Foundation OSS program requires product name
//! and version metadata set consistently across every signed binary. Rust
//! emits no VERSIONINFO by default, so before this script `dofek-tui.exe` (and
//! each plugin exe) had no ProductName, no FileVersion, and nothing for
//! Explorer's Details tab — while the Tauri-built GUI exe and MSI did. See
//! `docs/v1.6-signing-and-notarization.md`.
//!
//! Two deliberate choices:
//!
//! 1. **Failure warns, it does not abort.** There is no Windows toolchain on
//!    the machine this was written on, so the `rc.exe` invocation ships
//!    unverified. A hard failure here would break every Windows build,
//!    including the release job. What makes the soft failure safe is that
//!    `release.yml` *asserts* the metadata is present on each artifact before
//!    signing — so a silent failure cannot reach a release, it just fails
//!    loudly at the one point where it matters.
//! 2. **`ProductVersion` tracks the crate, not the app release.** The plugins
//!    version independently of dofek itself (0.1.x against 1.5.x), and a
//!    build script in a plugin crate has no honest way to learn the app's
//!    version. What SignPath needs is one consistent `ProductName` with
//!    versions present, which this gives; components of a signed suite
//!    routinely carry their own versions.
//!
//! `winresource` is declared under `[target.'cfg(windows)'.build-dependencies]`,
//! and for build dependencies that `cfg` is evaluated against the **host**.
//! So the crate is only fetched and compiled when building on Windows, and
//! non-Windows builds are untouched.

fn main() {
    // The resource is derived entirely from Cargo metadata.
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Expose the target triple to the crate. `std` has no equivalent, and
    // `plugin::store` needs it to recognise a Tauri `externalBin` sidecar,
    // which is staged as `<name>-<triple>` — see BUNDLED_PLUGINS.
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=DOFEK_TARGET_TRIPLE={target}");

    // Host-gated: `winresource` only exists as a dependency on Windows hosts.
    #[cfg(windows)]
    windows_version_info();
}

#[cfg(windows)]
fn windows_version_info() {
    // Target-gated as well as host-gated: building *for* Linux from a Windows
    // host must not try to embed a PE resource.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Dofek")
        .set("FileDescription", "Dofek — AI-aware system monitor (terminal UI)")
        .set("CompanyName", "Asaf Saar")
        .set("LegalCopyright", "Copyright (c) 2026 Asaf Saar")
        .set("OriginalFilename", "dofek-tui.exe");

    if let Err(e) = res.compile() {
        println!("cargo:warning=failed to embed Windows VERSIONINFO: {e}");
        println!(
            "cargo:warning=the binary will build, but release.yml's metadata \
             assertion will fail before signing"
        );
    }
}
