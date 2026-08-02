//! Shared text helpers for the TUI renderers.
//!
//! Every panel used to carry its own private `truncate`, and all six copies
//! byte-sliced a `&str` after checking `s.len()` (a *byte* count) against a
//! column budget. Any process, interface, or GPU name containing a multibyte
//! character could land the slice mid-codepoint and panic the whole TUI.
//! These helpers count characters instead, so the cut always falls on a
//! character boundary.
//!
//! Note: characters, not display columns. A CJK name still occupies two cells
//! per character, so a truncated-to-`max_len` string can overflow its column
//! visually. That is a layout imperfection, not a crash, and fixing it
//! properly means pulling in width tables — deliberately out of scope here.

/// Truncate `s` to at most `max_len` characters, marking the cut with
/// `ellipsis`. The result never exceeds `max_len` characters: the ellipsis is
/// budgeted from within it, and is dropped entirely when there isn't room.
pub fn truncate_with(s: &str, max_len: usize, ellipsis: &str) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    let ellipsis_len = ellipsis.chars().count();
    if max_len > ellipsis_len {
        let mut out: String = s.chars().take(max_len - ellipsis_len).collect();
        out.push_str(ellipsis);
        out
    } else {
        s.chars().take(max_len).collect()
    }
}

/// Truncate `s` to at most `max_len` characters with a `...` marker.
pub fn truncate(s: &str, max_len: usize) -> String {
    truncate_with(s, max_len, "...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_strings_pass_through() {
        assert_eq!(truncate("dofek", 10), "dofek");
        assert_eq!(truncate("dofek", 5), "dofek");
        assert_eq!(truncate("", 0), "");
    }

    #[test]
    fn long_ascii_gets_ellipsis_within_budget() {
        assert_eq!(truncate("abcdefghij", 8), "abcde...");
        assert_eq!(truncate_with("abcdefghij", 8, ".."), "abcdef..");
    }

    #[test]
    fn ellipsis_is_dropped_when_budget_is_too_small() {
        // Budget can't fit the marker, so we hard-cut instead.
        assert_eq!(truncate("abcdefg", 3), "abc");
        assert_eq!(truncate("abcdefg", 1), "a");
        assert_eq!(truncate("abcdefg", 0), "");
        assert_eq!(truncate_with("abcdefg", 2, ".."), "ab");
    }

    /// The regression that motivated this module: byte-slicing these panicked.
    #[test]
    fn multibyte_names_do_not_panic() {
        // Each CJK char is 3 bytes, so a byte-slice at any of these budgets
        // landed mid-codepoint.
        let cjk = "字幕処理サービス";
        for max in 0..=20 {
            assert!(truncate(cjk, max).chars().count() <= max);
        }
        assert_eq!(truncate(cjk, 5), "字幕...");

        // 4-byte codepoints (emoji) and combining sequences.
        let emoji = "🔥🔥🔥🔥🔥🔥";
        for max in 0..=12 {
            let _ = truncate(emoji, max);
        }
        assert_eq!(truncate(emoji, 5), "🔥🔥...");

        // Mixed ASCII + multibyte, cut exactly at the boundary.
        assert_eq!(truncate("py字torch", 6), "py字...");
    }

    #[test]
    fn multibyte_ellipsis_is_counted_in_chars_not_bytes() {
        assert_eq!(truncate_with("abcdefghij", 6, "…"), "abcde…");
        assert_eq!(truncate_with("abcdefghij", 1, "…"), "a");
    }

    #[test]
    fn output_never_exceeds_budget() {
        let cases = ["ascii-name", "字幕処理", "🔥emoji🔥", "a", ""];
        for s in cases {
            for max in 0..12 {
                for ell in ["...", "..", "…", ""] {
                    let out = truncate_with(s, max, ell);
                    assert!(
                        out.chars().count() <= max,
                        "truncate_with({s:?}, {max}, {ell:?}) = {out:?} exceeds budget"
                    );
                }
            }
        }
    }
}
