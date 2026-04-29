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

    Ok(UpdateInfo {
        current,
        latest: latest_clean,
        is_newer,
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
