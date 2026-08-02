//! Static guards on the GUI frontend's XSS posture.
//!
//! The GUI renders OS-controlled strings — a process can name itself
//! `<img src=x onerror=…>`. Two things keep that inert: a CSP that forbids
//! inline script, and escaping at every point where such a string is
//! interpolated into markup. Both are easy to regress silently in a
//! 1500-line hand-written frontend, so they are asserted here rather than
//! left to review.
//!
//! These are lexical checks over the source text, not a parse. They are
//! deliberately strict: the cost of a false positive is one `// SAFE:` line
//! explaining why an interpolation is inert, which is a comment worth having
//! anyway.

use std::path::{Path, PathBuf};

fn frontend_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/frontend")
}

fn read(rel: &str) -> String {
    let path = frontend_dir().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Every JS file the guards apply to.
const SCRIPTS: &[&str] = &["app.js", "overlays.js"];

/// One source line, tagged with whether it is inside a comment.
struct Line {
    number: usize,
    text: String,
    is_comment: bool,
}

/// Split `src` into lines, marking those that are entirely comment. Tracks
/// `/* … */` across lines — the row-cache commentary in app.js spans several
/// lines and mentions `innerHTML` in prose, which a prefix check would
/// misread as a sink.
fn scan(src: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut in_block = false;

    for (i, raw) in src.lines().enumerate() {
        let trimmed = raw.trim_start();
        let was_in_block = in_block;

        // Update block state from this line's delimiters. Good enough for a
        // hand-written frontend with no `/*` inside string literals.
        let mut rest = raw;
        while let Some(pos) = if in_block { rest.find("*/") } else { rest.find("/*") } {
            rest = &rest[pos + 2..];
            in_block = !in_block;
        }

        let is_comment = was_in_block || trimmed.starts_with("//") || trimmed.starts_with("/*");
        out.push(Line { number: i + 1, text: raw.to_string(), is_comment });
    }
    out
}

/// Assignments into `innerHTML` (and `insertAdjacentHTML`) must either
/// interpolate nothing external, or be annotated with a `// SAFE:` comment on
/// a preceding line stating why. The annotation is what makes a reviewer stop
/// and think; the test only enforces that they did.
#[test]
fn every_html_sink_is_annotated_safe() {
    let mut unannotated = Vec::new();

    for file in SCRIPTS {
        let lines = scan(&read(file));

        for (i, line) in lines.iter().enumerate() {
            if line.is_comment {
                continue; // prose that merely mentions innerHTML
            }
            let is_sink = (line.text.contains("innerHTML") && line.text.contains('='))
                || line.text.contains("insertAdjacentHTML");
            if !is_sink {
                continue;
            }

            // Look back over the immediately preceding comment block.
            let annotated = lines[..i]
                .iter()
                .rev()
                .take_while(|l| l.is_comment)
                .any(|l| l.text.contains("SAFE:"));

            if !annotated {
                unannotated.push(format!("{file}:{}: {}", line.number, line.text.trim()));
            }
        }
    }

    assert!(
        unannotated.is_empty(),
        "HTML sinks without a `// SAFE:` justification. Either write the value \
         with textContent, wrap it in esc(), or add a `// SAFE:` comment above \
         saying why it cannot carry external input:\n  {}",
        unannotated.join("\n  ")
    );
}

/// A process name must never be interpolated raw into a *markup* template.
///
/// Scoped to lines that actually build markup (they contain an opening tag),
/// so it doesn't fire on `sigFor`'s cache-key template or on `setAttribute`,
/// neither of which is an HTML parsing context.
#[test]
fn names_are_never_interpolated_into_markup() {
    const RAW_NAME: &[&str] = &[
        "${p.name}",
        "${g.name}",
        "${d.p.name}",
        "${sel.name}",
        "${targets[0].name}",
        "${title}",
        "${body}",
        "${info.latest}",
        "${preview}",
    ];

    let mut offenders = Vec::new();
    for file in SCRIPTS {
        for line in scan(&read(file)) {
            if line.is_comment {
                continue;
            }
            // Only lines that open an HTML tag are a parsing context.
            let builds_markup = ["<td", "<span", "<div", "<b>", "<button", "<p ", "<a "]
                .iter()
                .any(|t| line.text.contains(t));
            if !builds_markup {
                continue;
            }
            for raw in RAW_NAME {
                if line.text.contains(raw) {
                    offenders.push(format!("{file}:{}: {raw} in {}", line.number, line.text.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "external strings interpolated raw into markup. Write them with \
         textContent, or wrap in esc():\n  {}",
        offenders.join("\n  ")
    );
}

/// v1.7 removed `#[serde(skip)]` from `plugin_statuses`, so the dock now
/// renders strings a third-party binary chose. The `// SAFE:` escape hatch is
/// not good enough there: the whole renderer must be free of HTML sinks, so
/// there is no line for a future edit to add an interpolation to.
#[test]
fn plugin_dock_renderer_has_no_html_sinks_at_all() {
    let src = read("app.js");

    let lines = scan(&src);
    let marker = |needle: &str| {
        lines
            .iter()
            .position(|l| l.text.contains(needle))
            .unwrap_or_else(|| panic!("app.js must carry the `{needle}` marker the guard keys off"))
    };
    let start = marker("PLUGIN DOCK");
    let end = marker("END PLUGIN DOCK");
    assert!(end > start, "the END PLUGIN DOCK marker precedes the opening banner");

    // Comments are excluded — the section's own commentary explains *why* it
    // avoids innerHTML, and naming the sink shouldn't trip the guard.
    let code: Vec<&Line> = lines[start..end].iter().filter(|l| !l.is_comment).collect();

    assert!(
        code.iter().any(|l| l.text.contains("function upPlugins")),
        "the PLUGIN DOCK section no longer contains upPlugins() — the guard is \
         scanning the wrong range"
    );

    for sink in ["innerHTML", "insertAdjacentHTML", "outerHTML", "document.write"] {
        if let Some(l) = code.iter().find(|l| l.text.contains(sink)) {
            panic!(
                "app.js:{}: the plugin dock renderer uses `{sink}`. Plugin output \
                 is third-party text — build the row with createElement and write \
                 values with textContent.\n  {}",
                l.number,
                l.text.trim()
            );
        }
    }
    assert!(
        code.iter().any(|l| l.text.contains("textContent")),
        "the plugin dock renderer must write its values with textContent"
    );
}

/// The renderer is only reached if it's in the frame loop; a dock that never
/// repaints looks exactly like a dock with no plugins.
#[test]
fn plugin_dock_is_wired_into_the_frame_loop() {
    let src = read("app.js");
    let loop_line = src
        .lines()
        .find(|l| l.contains("const fns=[") || l.contains("const fns = ["))
        .expect("app.js must build the per-frame renderer list");
    assert!(
        loop_line.contains("upPlugins"),
        "upPlugins is not in the frame loop: {}",
        loop_line.trim()
    );
}

/// `plugin_label` is plugin-controlled and lands in a process row. The row
/// builder does use `innerHTML` (for the fixed cell scaffolding), so this
/// asserts the label itself is not part of that template.
#[test]
fn plugin_label_is_never_interpolated_into_the_row_template() {
    for file in SCRIPTS {
        for line in scan(&read(file)) {
            if line.is_comment {
                continue;
            }
            let builds_markup = line.text.contains("innerHTML") || line.text.contains("<span");
            if builds_markup {
                assert!(
                    !line.text.contains("${p.label}") && !line.text.contains("${d.p.label}"),
                    "{file}:{}: plugin label interpolated into markup — write it \
                     with textContent:\n  {}",
                    line.number,
                    line.text.trim()
                );
            }
        }
    }
    // …and it must actually be written somewhere, or the badge is dead markup.
    let src = read("app.js");
    assert!(
        src.contains(".plabel"),
        "app.js never touches the .plabel badge — plugin_label is still unrendered"
    );
}

/// `esc()` must exist, cover both element and attribute context, and be
/// applied somewhere — a helper nobody calls is not a fix.
#[test]
fn esc_helper_exists_and_is_used() {
    let src = read("app.js");
    assert!(src.contains("function esc("), "app.js must define esc()");

    for ch in ['&', '<', '>', '"', '\''] {
        let entity = match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            _ => "&#39;",
        };
        assert!(
            src.contains(entity),
            "esc() must escape {ch:?} to {entity} — attribute context needs \
             quotes escaped, not just angle brackets"
        );
    }

    let uses: usize = SCRIPTS.iter().map(|f| read(f).matches("esc(").count()).sum();
    assert!(uses > 5, "esc() is defined but barely used ({uses} occurrences)");
}

/// The real fix. Inline script must stay forbidden: with `script-src 'self'`,
/// markup injected into the DOM cannot execute.
#[test]
fn csp_forbids_inline_script() {
    let conf = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/tauri.conf.json"),
    )
    .expect("gui/tauri.conf.json should be readable");
    let json: serde_json::Value = serde_json::from_str(&conf).expect("tauri.conf.json is JSON");

    let csp = json["app"]["security"]["csp"]
        .as_str()
        .expect("app.security.csp must be set");

    let script_src = csp
        .split(';')
        .map(str::trim)
        .find(|d| d.starts_with("script-src"))
        .expect("CSP must declare script-src explicitly, not inherit default-src");

    assert!(
        !script_src.contains("unsafe-inline"),
        "script-src must not allow 'unsafe-inline' — that is what turns a DOM \
         injection into code execution. Found: {script_src}"
    );
    assert!(
        !script_src.contains("unsafe-eval"),
        "script-src must not allow 'unsafe-eval'. Found: {script_src}"
    );
}

/// `script-src 'self'` only holds if there is no inline script left to block.
/// An inline block would silently stop running rather than fail loudly, so
/// catch it here instead of at runtime.
#[test]
fn index_html_has_no_inline_script_or_handlers() {
    let html = read("index.html");

    // `<script>` with no src attribute is an inline block.
    for (i, line) in html.lines().enumerate() {
        if let Some(pos) = line.find("<script") {
            let tag_end = line[pos..].find('>').map(|e| pos + e).unwrap_or(line.len());
            let tag = &line[pos..tag_end];
            assert!(
                tag.contains("src="),
                "index.html:{}: inline <script> would be blocked by \
                 script-src 'self'. Move it to a .js file.\n  {}",
                i + 1,
                line.trim()
            );
        }
    }

    // Inline event-handler attributes are also inline script under CSP.
    for handler in [
        " onclick=", " onchange=", " oninput=", " onload=", " onerror=",
        " onsubmit=", " onkeydown=", " onkeyup=", " onmouseover=", " onfocus=",
    ] {
        assert!(
            !html.contains(handler),
            "index.html uses an inline `{}` attribute, which \
             script-src 'self' blocks. Bind it with addEventListener instead.",
            handler.trim()
        );
    }

    assert!(
        !html.contains("javascript:"),
        "index.html contains a `javascript:` URL, which CSP blocks"
    );
}

/// The extracted files must actually be referenced, or the UI silently loses
/// all behaviour and styling.
#[test]
fn index_html_references_the_extracted_assets() {
    let html = read("index.html");
    for asset in ["app.css", "app.js", "overlays.js"] {
        assert!(
            html.contains(asset),
            "index.html no longer references {asset}"
        );
        assert!(
            frontend_dir().join(asset).is_file(),
            "{asset} is referenced by index.html but missing from gui/frontend/"
        );
    }
}
