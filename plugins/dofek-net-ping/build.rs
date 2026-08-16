//! Embeds a Windows VERSIONINFO resource. Required by the SignPath Foundation
//! program, which needs product name and version metadata consistent across
//! every signed binary — see the long rationale in the workspace-root
//! `build.rs` and `docs/v1.6-signing-and-notarization.md`.
//!
//! Failure warns rather than aborting; `release.yml` asserts the metadata is
//! present before signing, so a silent failure cannot reach a release.

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");

    #[cfg(windows)]
    windows_version_info();
}

#[cfg(windows)]
fn windows_version_info() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Dofek")
        .set("FileDescription", "Dofek plugin: TCP-connect latency to a remote host")
        .set("CompanyName", "Asaf Saar")
        .set("LegalCopyright", "Copyright (c) 2026 Asaf Saar")
        .set("OriginalFilename", "dofek-net-ping.exe");

    if let Err(e) = res.compile() {
        println!("cargo:warning=failed to embed Windows VERSIONINFO: {e}");
    }
}
