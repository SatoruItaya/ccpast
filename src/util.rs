#![allow(dead_code)]

use chrono::{DateTime, Local, Utc};
use unicode_width::UnicodeWidthChar;

/// Truncate `s` so that its displayed width does not exceed `max_cols`.
/// If truncated, the last visible glyph is replaced with `…`.
/// Returns the original string when it already fits.
pub fn truncate_to_width(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let mut width = 0usize;
    let mut out = String::new();
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > max_cols {
            // Need to leave room for the ellipsis (1 col).
            while width > max_cols.saturating_sub(1) {
                let last = match out.pop() {
                    Some(c) => c,
                    None => break,
                };
                width -= UnicodeWidthChar::width(last).unwrap_or(0);
            }
            out.push('…');
            return out;
        }
        width += w;
        out.push(ch);
    }
    out
}

/// Render a UTC `DateTime` as `YYYY-MM-DD HH:MM` in local time.
pub fn format_local_short(ts: DateTime<Utc>) -> String {
    ts.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn truncate_returns_input_when_within_width() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn truncate_handles_ascii_overflow() {
        assert_eq!(truncate_to_width("hello world", 8), "hello w…");
    }

    #[test]
    fn truncate_counts_multibyte_visual_width() {
        // 5 wide characters take 10 cols; we ask for 6 cols, expect "あい…" (5 cols total).
        let out = truncate_to_width("あいうえお", 6);
        assert_eq!(out, "あい…");
    }

    #[test]
    fn truncate_zero_width_returns_empty() {
        assert_eq!(truncate_to_width("hello", 0), "");
    }

    #[test]
    fn format_local_short_renders_known_instant() {
        let ts = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let out = format_local_short(ts);
        // Locale-dependent; just verify shape "YYYY-MM-DD HH:MM"
        assert_eq!(out.len(), 16);
        assert_eq!(&out[4..5], "-");
        assert_eq!(&out[7..8], "-");
        assert_eq!(&out[10..11], " ");
        assert_eq!(&out[13..14], ":");
    }
}
