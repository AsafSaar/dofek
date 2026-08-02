//! Update checker — queries the GitHub Releases API for the latest Dofek
//! release and compares it against the compiled-in `CARGO_PKG_VERSION`.
//!
//! Notify-only: this module never downloads or installs anything. It just
//! tells the caller whether a newer version exists and where to find it.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const REPO: &str = "AsafSaar/dofek";
const RELEASES_URL: &str = "https://api.github.com/repos/AsafSaar/dofek/releases/latest";
const NOTES_MAX_CHARS: usize = 600;
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub is_newer: bool,
    pub url: String,
    pub notes: String,
    /// How this copy of dofek was installed, inferred from `current_exe()`.
    pub channel: InstallChannel,
    /// One-line, channel-appropriate instruction for actually getting the
    /// update — `brew upgrade` for a Homebrew install rather than "download the
    /// dmg", which would leave two copies on disk fighting over the same config.
    pub hint: String,
}

/// Where this binary appears to have been installed from.
///
/// Inferred from the executable's path, which is a heuristic — but a cheap and
/// stable one, and being wrong only means showing a less specific hint. There is
/// no manifest a package manager leaves behind that all of these agree on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallChannel {
    /// Homebrew formula or cask.
    Homebrew,
    /// Windows Package Manager.
    WinGet,
    /// Distro package — installed under a system prefix by dpkg/rpm.
    SystemPackage,
    /// Running from an extracted AppImage.
    AppImage,
    /// A build tree — `cargo run` / `target/{debug,release}`.
    Development,
    /// Anything else: MSI, `.dmg`, or a manually placed binary. This is the
    /// only channel v1.7's in-app updater will ever serve.
    Standalone,
}

impl InstallChannel {
    /// Whether an in-app update is appropriate for this channel. A package
    /// manager owns its files; writing over them from inside the app leaves the
    /// manager's database lying about what is installed.
    pub fn supports_in_app_update(self) -> bool {
        matches!(self, InstallChannel::Standalone | InstallChannel::AppImage)
    }
}

/// Infer the install channel from the running executable's path.
pub fn detect_channel() -> InstallChannel {
    match std::env::current_exe() {
        Ok(path) => channel_for_path(&path),
        // Without a path there is nothing to infer from; the generic hint is
        // correct for the fallback.
        Err(_) => InstallChannel::Standalone,
    }
}

/// The path-matching half of [`detect_channel`], split out so it can be tested
/// against fixed paths instead of wherever the test binary happens to live.
fn channel_for_path(path: &std::path::Path) -> InstallChannel {
    // Compare with forward slashes throughout: it keeps one set of patterns for
    // both platforms, and Windows accepts either separator anyway.
    let p = path.to_string_lossy().replace('\\', "/");
    let lower = p.to_lowercase();

    // Development first — a build tree can sit anywhere, including under a
    // path that would otherwise match a system prefix.
    if p.contains("/target/debug/") || p.contains("/target/release/") {
        return InstallChannel::Development;
    }
    // AppImage exports APPDIR and mounts read-only; the path alone is a
    // temporary mountpoint, so the env var is the reliable signal.
    if std::env::var_os("APPDIR").is_some() {
        return InstallChannel::AppImage;
    }
    // Homebrew: /opt/homebrew (Apple Silicon), /usr/local/Homebrew (Intel),
    // and the Cellar/Caskroom paths symlinks point into.
    if lower.contains("/homebrew/") || lower.contains("/cellar/") || lower.contains("/caskroom/") {
        return InstallChannel::Homebrew;
    }
    if lower.contains("/winget/packages/") || lower.contains("/microsoft/winget/") {
        return InstallChannel::WinGet;
    }
    // Distro packages land under a system prefix. Checked after Homebrew
    // because Intel Homebrew lives under /usr/local.
    if p.starts_with("/usr/bin/") || p.starts_with("/usr/local/bin/") || p.starts_with("/opt/") {
        return InstallChannel::SystemPackage;
    }
    InstallChannel::Standalone
}

/// The channel-appropriate one-liner shown alongside "an update is available".
fn hint_for(channel: InstallChannel, latest: &str) -> String {
    match channel {
        InstallChannel::Homebrew => {
            "Installed via Homebrew — run `brew upgrade --cask dofek` \
             (or `brew upgrade dofek` for the CLI formula)."
                .to_string()
        }
        InstallChannel::WinGet => {
            "Installed via WinGet — run `winget upgrade AsafSaar.dofek`.".to_string()
        }
        InstallChannel::SystemPackage => format!(
            "Installed from a system package — download the {latest} .deb/.rpm from the \
             releases page and install it with your package manager."
        ),
        InstallChannel::AppImage => {
            "Running from an AppImage — download the new AppImage and replace this file.".to_string()
        }
        InstallChannel::Development => {
            "Running from a build tree — `git pull` and rebuild.".to_string()
        }
        InstallChannel::Standalone => {
            "Download the installer for your platform from the releases page.".to_string()
        }
    }
}

/// Synchronously query GitHub for the latest release. Returns `UpdateInfo`
/// with `is_newer = false` when the local build is at or ahead of the latest
/// tag — callers can present a "you're up to date" message in that case.
pub fn check() -> Result<UpdateInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    // GitHub rejects unauthenticated API requests without a User-Agent.
    let user_agent = format!("dofek/{current} (+https://github.com/{REPO})");

    let resp = ureq::get(RELEASES_URL)
        .set("User-Agent", &user_agent)
        .set("Accept", "application/vnd.github+json")
        .timeout(HTTP_TIMEOUT)
        .call()
        .context("GitHub Releases API request failed")?;

    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
        html_url: String,
        #[serde(default)]
        body: String,
    }
    let body = resp.into_string().context("reading GitHub response body")?;
    let rel: Release = serde_json::from_str(&body).context("parsing GitHub release JSON")?;

    let latest_clean = rel.tag_name.trim_start_matches('v').to_string();
    let is_newer = is_strictly_newer(&latest_clean, &current)
        .ok_or_else(|| anyhow!("could not parse versions: {current} vs {latest_clean}"))?;

    let channel = detect_channel();
    Ok(UpdateInfo {
        current,
        hint: hint_for(channel, &latest_clean),
        latest: latest_clean,
        is_newer,
        channel,
        url: rel.html_url,
        notes: truncate_notes(&rel.body, NOTES_MAX_CHARS),
    })
}

/// Returns `Some(true)` iff `latest` > `current` under MAJOR.MINOR.PATCH
/// ordering. Pre-release suffixes (`-rc1`, `+build`) are stripped before
/// comparing — pre-releases of the same version are treated as equal to the
/// release itself, which avoids nagging users on RC builds.
fn is_strictly_newer(latest: &str, current: &str) -> Option<bool> {
    let l = parse_semver(latest)?;
    let c = parse_semver(current)?;
    Some(l > c)
}

fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn truncate_notes(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_ordering() {
        assert_eq!(is_strictly_newer("1.3.5", "1.3.4"), Some(true));
        assert_eq!(is_strictly_newer("1.3.4", "1.3.4"), Some(false));
        assert_eq!(is_strictly_newer("1.3.3", "1.3.4"), Some(false));
        assert_eq!(is_strictly_newer("2.0.0", "1.99.99"), Some(true));
        assert_eq!(is_strictly_newer("1.4.0", "1.3.99"), Some(true));
    }

    #[test]
    fn semver_handles_prerelease_and_short_forms() {
        assert_eq!(is_strictly_newer("1.4.0-rc1", "1.3.4"), Some(true));
        // Same base version, pre-release tag — treated as equal, not newer.
        assert_eq!(is_strictly_newer("1.3.4-rc1", "1.3.4"), Some(false));
        // Two-component versions (no patch) parse as patch=0.
        assert_eq!(is_strictly_newer("1.4", "1.3.9"), Some(true));
    }

    #[test]
    fn semver_rejects_garbage() {
        assert_eq!(is_strictly_newer("not-a-version", "1.0.0"), None);
    }

    use std::path::Path;

    /// v1.7 needs to know how dofek was installed, because telling a Homebrew
    /// user to download a .dmg leaves two copies fighting over one config
    /// directory — and because the in-app updater must never touch files a
    /// package manager owns.
    #[test]
    fn install_channel_is_inferred_from_the_executable_path() {
        let cases: &[(&str, InstallChannel)] = &[
            // Homebrew, both prefixes and the paths its symlinks point into.
            ("/opt/homebrew/bin/dofek-tui", InstallChannel::Homebrew),
            ("/opt/homebrew/Cellar/dofek/1.5.1/bin/dofek-tui", InstallChannel::Homebrew),
            ("/usr/local/Caskroom/dofek/1.5.1/Dofek.app/Contents/MacOS/dofek-gui", InstallChannel::Homebrew),
            ("/usr/local/Homebrew/bin/dofek-tui", InstallChannel::Homebrew),
            // WinGet.
            (r"C:\Users\a\AppData\Local\Microsoft\WinGet\Packages\AsafSaar.dofek\dofek-tui.exe", InstallChannel::WinGet),
            // Distro packages.
            ("/usr/bin/dofek-tui", InstallChannel::SystemPackage),
            ("/opt/dofek/dofek-tui", InstallChannel::SystemPackage),
            // Build trees.
            ("/Users/a/dev/dofek/target/release/dofek-tui", InstallChannel::Development),
            ("/Users/a/dev/dofek/target/debug/dofek-tui", InstallChannel::Development),
            // Everything else: MSI, .dmg, hand-placed.
            (r"C:\Program Files\Dofek\dofek-gui.exe", InstallChannel::Standalone),
            ("/Applications/Dofek.app/Contents/MacOS/dofek-gui", InstallChannel::Standalone),
            ("/Users/a/Downloads/dofek-tui", InstallChannel::Standalone),
        ];

        for (path, want) in cases {
            assert_eq!(
                channel_for_path(Path::new(path)),
                *want,
                "misclassified {path}"
            );
        }
    }

    /// Intel Homebrew lives under `/usr/local`, which also matches the system
    /// prefix rule — so the Homebrew check has to come first. This is the
    /// ordering bug the test exists to pin.
    #[test]
    fn intel_homebrew_beats_the_system_prefix_rule() {
        assert_eq!(
            channel_for_path(Path::new("/usr/local/Homebrew/bin/dofek-tui")),
            InstallChannel::Homebrew
        );
        // A genuine /usr/local install with no Homebrew marker is still a
        // system package.
        assert_eq!(
            channel_for_path(Path::new("/usr/local/bin/dofek-tui")),
            InstallChannel::SystemPackage
        );
    }

    /// A build tree can sit anywhere, including under a path that would
    /// otherwise look like a system prefix.
    #[test]
    fn a_build_tree_under_a_system_prefix_is_still_development() {
        assert_eq!(
            channel_for_path(Path::new("/opt/ci/dofek/target/release/dofek-tui")),
            InstallChannel::Development
        );
    }

    /// The hint is the whole point — every channel must produce a non-empty,
    /// channel-specific instruction, and the package-manager ones must name
    /// the actual command.
    #[test]
    fn every_channel_has_an_actionable_hint() {
        for ch in [
            InstallChannel::Homebrew,
            InstallChannel::WinGet,
            InstallChannel::SystemPackage,
            InstallChannel::AppImage,
            InstallChannel::Development,
            InstallChannel::Standalone,
        ] {
            let hint = hint_for(ch, "1.6.0");
            assert!(!hint.is_empty(), "{ch:?} has no hint");
        }
        assert!(hint_for(InstallChannel::Homebrew, "1.6.0").contains("brew upgrade"));
        assert!(hint_for(InstallChannel::WinGet, "1.6.0").contains("winget upgrade"));
        assert!(hint_for(InstallChannel::SystemPackage, "1.6.0").contains("1.6.0"));
    }

    /// Package-manager installs must be excluded from the in-app updater:
    /// writing over dpkg/brew-owned files leaves their databases lying about
    /// what is installed. This gates v1.7's updater wiring.
    #[test]
    fn only_self_contained_channels_allow_in_app_update() {
        assert!(InstallChannel::Standalone.supports_in_app_update());
        assert!(InstallChannel::AppImage.supports_in_app_update());

        for ch in [
            InstallChannel::Homebrew,
            InstallChannel::WinGet,
            InstallChannel::SystemPackage,
            InstallChannel::Development,
        ] {
            assert!(
                !ch.supports_in_app_update(),
                "{ch:?} must not be offered an in-app update"
            );
        }
    }

    /// The GUI reads the channel off the serialized payload.
    #[test]
    fn channel_serializes_lowercase() {
        for (ch, want) in [
            (InstallChannel::Homebrew, "\"homebrew\""),
            (InstallChannel::WinGet, "\"winget\""),
            (InstallChannel::SystemPackage, "\"systempackage\""),
            (InstallChannel::AppImage, "\"appimage\""),
            (InstallChannel::Development, "\"development\""),
            (InstallChannel::Standalone, "\"standalone\""),
        ] {
            assert_eq!(serde_json::to_string(&ch).unwrap(), want);
        }
    }

    /// Live network smoke test — disabled by default. Run with:
    ///   cargo test --lib update::tests::live_check -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_check() {
        let info = check().expect("live update check should succeed");
        println!("live check: {info:#?}");
        assert!(!info.latest.is_empty());
        assert!(info.url.starts_with("https://"));
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate_notes("hello", 10), "hello");
        assert_eq!(truncate_notes("0123456789abcdef", 10), "0123456789…");
    }
}
