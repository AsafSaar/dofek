//! Bounds every plugin-supplied string and collection **at ingest**, before a
//! `PollResponse` is stored anywhere the rest of the program can see it.
//!
//! Why here and not at the render sites: a renderer that calls `.take(6)` is
//! bounding *what it draws*, not what the process is holding. A plugin that
//! answers with 200 000 panels every 500 ms still grows the heap without
//! limit, still costs a full clone on every `statuses()` call, and — once
//! `plugin_statuses` loses its `#[serde(skip)]` (v1.7) — still ships megabytes
//! into the webview on every tick. Capping at the boundary means every
//! consumer downstream is safe by construction and none of them has to know
//! the limits.
//!
//! The caps are deliberately generous relative to what any real plugin sends
//! (the three first-party ones use 1 panel, ≤6 entries, ≤2 metrics) and small
//! enough that a hostile or broken plugin's response stays in the low
//! kilobytes. Truncation is silent-but-logged: a plugin author who overruns
//! sees it in `--debug`, and a user never sees a broken UI.
//!
//! This module is what makes `SECURITY.md`'s "output size limits are enforced"
//! claim true. It pairs with the 256 KiB per-line read cap in
//! [`super::worker`] — that one bounds a single *read*, this one bounds what
//! survives parsing.

use super::protocol::{
    Metric, Panel, PanelEntry, PluginManifest, PollResponse, ProcessAnnotation,
};

/// Maximum panels kept from one response.
pub const MAX_PANELS: usize = 8;
/// Maximum entries kept per panel.
pub const MAX_PANEL_ENTRIES: usize = 16;
/// Maximum ticker metrics kept from one response.
pub const MAX_METRICS: usize = 8;
/// Maximum process annotations kept from one response. Higher than the other
/// caps because there is one legitimate annotation per process, and a busy
/// machine genuinely has hundreds.
pub const MAX_ANNOTATIONS: usize = 512;

/// Cap for identifiers and labels — things that sit in a column or a pill.
pub const MAX_SHORT_CHARS: usize = 64;
/// Cap for free-form values and descriptions.
pub const MAX_LONG_CHARS: usize = 128;

/// Clean one plugin-supplied string: strip control characters (which includes
/// `\r` and `\n`, the two that could forge extra protocol lines or corrupt a
/// terminal), then truncate to `max_chars` on a character boundary.
///
/// Character-based, not display-width-based — same tradeoff as
/// [`crate::ui::text`], and for the same reason.
fn clean(s: &str, max_chars: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

/// [`clean`] at the identifier/label cap. Exposed for the install-time
/// manifest probe, which writes plugin-supplied strings into `plugins.toml`
/// and both UIs without going through [`sanitize_response`].
pub fn clean_short(s: &str) -> String {
    clean(s, MAX_SHORT_CHARS)
}

/// [`clean`] at the free-text cap.
pub fn clean_long(s: &str) -> String {
    clean(s, MAX_LONG_CHARS)
}

/// Returns `true` if `s` needed no cleaning. Used only to decide whether to
/// log; the cleaned value is what gets stored either way.
fn is_clean(s: &str, max_chars: usize) -> bool {
    let mut n = 0;
    for c in s.chars() {
        if c.is_control() {
            return false;
        }
        n += 1;
        if n > max_chars {
            return false;
        }
    }
    true
}

/// Sanitize a freshly parsed response in place, returning the number of
/// individual items dropped or truncated so the caller can log once instead
/// of once per field.
pub fn sanitize_response(resp: &mut PollResponse, plugin: &str) {
    let mut dropped = 0usize;

    if let Some(m) = resp.manifest.as_mut() {
        dropped += sanitize_manifest(m);
    }

    dropped += truncate_vec(&mut resp.panels, MAX_PANELS);
    for p in &mut resp.panels {
        dropped += sanitize_panel(p);
    }

    dropped += truncate_vec(&mut resp.metrics, MAX_METRICS);
    for m in &mut resp.metrics {
        dropped += sanitize_metric(m);
    }

    dropped += truncate_vec(&mut resp.process_annotations, MAX_ANNOTATIONS);
    for a in &mut resp.process_annotations {
        dropped += sanitize_annotation(a);
    }

    if !is_clean(&resp.status, MAX_SHORT_CHARS) {
        dropped += 1;
    }
    resp.status = clean(&resp.status, MAX_SHORT_CHARS);

    if dropped > 0 {
        log::debug!("plugin '{plugin}': {dropped} response field(s) truncated or dropped by the sanitizer");
    }
}

/// Truncate `v` to `max`, returning how many elements were discarded.
fn truncate_vec<T>(v: &mut Vec<T>, max: usize) -> usize {
    if v.len() > max {
        let dropped = v.len() - max;
        v.truncate(max);
        dropped
    } else {
        0
    }
}

/// Clean `field` in place, counting it if it changed.
fn clean_field(field: &mut String, max_chars: usize, dropped: &mut usize) {
    if !is_clean(field, max_chars) {
        *dropped += 1;
        *field = clean(field, max_chars);
    }
}

fn sanitize_manifest(m: &mut PluginManifest) -> usize {
    let mut d = 0;
    clean_field(&mut m.name, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut m.version, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut m.author, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut m.description, MAX_LONG_CHARS, &mut d);
    d
}

fn sanitize_panel(p: &mut Panel) -> usize {
    let mut d = 0;
    clean_field(&mut p.id, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut p.label, MAX_SHORT_CHARS, &mut d);
    d += truncate_vec(&mut p.content, MAX_PANEL_ENTRIES);
    for e in &mut p.content {
        d += sanitize_entry(e);
    }
    d
}

fn sanitize_entry(e: &mut PanelEntry) -> usize {
    let mut d = 0;
    clean_field(&mut e.key, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut e.value, MAX_LONG_CHARS, &mut d);
    clean_field(&mut e.style, MAX_SHORT_CHARS, &mut d);
    d
}

fn sanitize_metric(m: &mut Metric) -> usize {
    let mut d = 0;
    clean_field(&mut m.id, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut m.label, MAX_SHORT_CHARS, &mut d);
    clean_field(&mut m.unit, MAX_SHORT_CHARS, &mut d);
    // A non-finite value formats as "NaN"/"inf" in the ticker and serializes
    // as JSON `null` into the GUI — same class of bug `data::rate` guards.
    if !m.value.is_finite() {
        m.value = 0.0;
        d += 1;
    }
    d
}

fn sanitize_annotation(a: &mut ProcessAnnotation) -> usize {
    let mut d = 0;
    if let Some(s) = a.label.as_mut() {
        clean_field(s, MAX_SHORT_CHARS, &mut d);
    }
    if let Some(s) = a.category.as_mut() {
        clean_field(s, MAX_SHORT_CHARS, &mut d);
    }
    if let Some(s) = a.ai_state.as_mut() {
        clean_field(s, MAX_SHORT_CHARS, &mut d);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, value: &str) -> PanelEntry {
        PanelEntry {
            key: key.into(),
            value: value.into(),
            style: "normal".into(),
        }
    }

    fn panel(id: &str, n_entries: usize) -> Panel {
        Panel {
            id: id.into(),
            label: id.to_uppercase(),
            content: (0..n_entries).map(|i| entry(&format!("k{i}"), "v")).collect(),
        }
    }

    #[test]
    fn a_well_behaved_response_is_untouched() {
        let mut resp = PollResponse {
            status: "ok".into(),
            panels: vec![panel("ollama", 4)],
            metrics: vec![Metric {
                id: "tok_s".into(),
                label: "TOK/S".into(),
                value: 42.5,
                unit: "/s".into(),
            }],
            ..Default::default()
        };
        let before = format!("{resp:?}");
        sanitize_response(&mut resp, "test");
        assert_eq!(before, format!("{resp:?}"), "sanitizer must be a no-op on legal input");
    }

    #[test]
    fn collections_are_capped() {
        let mut resp = PollResponse {
            panels: (0..1000).map(|i| panel(&format!("p{i}"), 1000)).collect(),
            metrics: (0..1000)
                .map(|i| Metric {
                    id: format!("m{i}"),
                    label: "M".into(),
                    value: 0.0,
                    unit: String::new(),
                })
                .collect(),
            process_annotations: (0..10_000)
                .map(|i| ProcessAnnotation {
                    pid: i,
                    label: Some("x".into()),
                    category: None,
                    ai_state: None,
                })
                .collect(),
            ..Default::default()
        };
        sanitize_response(&mut resp, "test");

        assert_eq!(resp.panels.len(), MAX_PANELS);
        assert!(resp.panels.iter().all(|p| p.content.len() <= MAX_PANEL_ENTRIES));
        assert_eq!(resp.metrics.len(), MAX_METRICS);
        assert_eq!(resp.process_annotations.len(), MAX_ANNOTATIONS);
    }

    /// The `--giant-line` fixture's shape: one legal-looking panel carrying a
    /// multi-megabyte value.
    #[test]
    fn a_giant_value_is_cut_to_the_string_cap() {
        let mut resp = PollResponse {
            panels: vec![Panel {
                id: "giant".into(),
                label: "GIANT".into(),
                content: vec![entry("blob", &"A".repeat(4 * 1024 * 1024))],
            }],
            ..Default::default()
        };
        sanitize_response(&mut resp, "test");
        assert_eq!(resp.panels[0].content[0].value.chars().count(), MAX_LONG_CHARS);
    }

    /// `\n` would let a plugin forge an extra protocol line downstream; `\r`
    /// and ANSI escapes can rewrite the terminal from inside a panel.
    #[test]
    fn control_characters_are_stripped() {
        let mut resp = PollResponse {
            status: "ok\r\n{\"status\":\"forged\"}".into(),
            panels: vec![Panel {
                id: "p".into(),
                label: "\x1b[2J\x1b[HWIPED".into(),
                content: vec![entry("k\ty", "line1\nline2\0nul")],
            }],
            ..Default::default()
        };
        sanitize_response(&mut resp, "test");

        assert_eq!(resp.status, "ok{\"status\":\"forged\"}");
        assert_eq!(resp.panels[0].label, "[2J[HWIPED");
        assert_eq!(resp.panels[0].content[0].key, "ky");
        assert_eq!(resp.panels[0].content[0].value, "line1line2nul");

        let all = format!("{resp:?}");
        for bad in ['\n', '\r', '\0', '\x1b', '\t'] {
            assert!(!resp.status.contains(bad), "status kept {bad:?}");
            assert!(
                !resp.panels[0].label.contains(bad) && !resp.panels[0].content[0].value.contains(bad),
                "panel kept {bad:?} in {all}"
            );
        }
    }

    /// Truncation must not split a codepoint — the same class of bug as
    /// finding #3 in the TUI renderers.
    #[test]
    fn truncation_is_char_boundary_safe() {
        let cjk = "字".repeat(1000);
        let mut resp = PollResponse {
            panels: vec![Panel {
                id: cjk.clone(),
                label: "🔥".repeat(500),
                content: vec![entry(&cjk, &cjk)],
            }],
            ..Default::default()
        };
        sanitize_response(&mut resp, "test");
        assert_eq!(resp.panels[0].id.chars().count(), MAX_SHORT_CHARS);
        assert_eq!(resp.panels[0].label.chars().count(), MAX_SHORT_CHARS);
        assert_eq!(resp.panels[0].content[0].value.chars().count(), MAX_LONG_CHARS);
        // Round-tripping proves no partial codepoint survived.
        assert!(resp.panels[0].id.is_char_boundary(resp.panels[0].id.len()));
    }

    #[test]
    fn non_finite_metrics_are_zeroed() {
        let mut resp = PollResponse {
            metrics: vec![
                Metric { id: "a".into(), label: "A".into(), value: f64::NAN, unit: String::new() },
                Metric { id: "b".into(), label: "B".into(), value: f64::INFINITY, unit: String::new() },
                Metric { id: "c".into(), label: "C".into(), value: 1.5, unit: String::new() },
            ],
            ..Default::default()
        };
        sanitize_response(&mut resp, "test");
        assert_eq!(resp.metrics[0].value, 0.0);
        assert_eq!(resp.metrics[1].value, 0.0);
        assert_eq!(resp.metrics[2].value, 1.5);
    }

    /// Sanitizing twice must give the same answer as sanitizing once —
    /// otherwise a value could drift each tick.
    #[test]
    fn sanitizing_is_idempotent() {
        let mut resp = PollResponse {
            status: "\u{7}bad".into(),
            panels: (0..50).map(|i| panel(&format!("p{i}"), 50)).collect(),
            ..Default::default()
        };
        sanitize_response(&mut resp, "test");
        let once = format!("{resp:?}");
        sanitize_response(&mut resp, "test");
        assert_eq!(once, format!("{resp:?}"));
    }
}
