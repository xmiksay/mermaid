//! Shared text-metric estimates.
//!
//! A static renderer can't measure real glyph boxes the way upstream Mermaid
//! does through the DOM, so text width is estimated as `chars × per-glyph
//! width`. That per-glyph width — and the line height — must track the
//! configured font size, otherwise every node box stays tuned to the default
//! 14px and text overflows its shape once `--font-size` grows.
//!
//! Renderers keep their own `base_char_w` (a stick-figure actor label reads a
//! touch wider than a flowchart node, bold PK/FK keys wider still); this module
//! owns the one thing they all got wrong — making that width proportional to
//! `font_size`.

/// Font size (px) the per-renderer width constants are tuned against.
pub(crate) const BASE_FONT_SIZE: f64 = 14.0;

/// Scale factor mapping a metric tuned at [`BASE_FONT_SIZE`] to `font_size`.
pub(crate) fn font_scale(font_size: f64) -> f64 {
    font_size / BASE_FONT_SIZE
}

/// Estimated pixel width of one line of `s`, where `base_char_w` is the
/// renderer's per-glyph width at [`BASE_FONT_SIZE`]. Grows with `font_size`.
///
/// CJK / full-width glyphs occupy roughly one em (≈ two half-width glyphs), so
/// counting them as a single `base_char_w` unit underestimates a CJK label by
/// ~50% and text overflows its shape; they are counted as **two** units (#187).
pub(crate) fn text_width(s: &str, base_char_w: f64, font_size: f64) -> f64 {
    let units: f64 = s.chars().map(char_width_units).sum();
    units * base_char_w * font_scale(font_size)
}

/// Advance width of `c` in half-width units: 2 for East-Asian wide / full-width
/// glyphs, 1 otherwise. Covers the common CJK ideograph, kana, Hangul and
/// full-width-form ranges — enough to size labels without a full Unicode table.
fn char_width_units(c: char) -> f64 {
    let u = c as u32;
    let wide = matches!(u,
        0x1100..=0x115F |   // Hangul Jamo
        0x2E80..=0x303E |   // CJK radicals, Kangxi, CJK symbols/punctuation
        0x3041..=0x33FF |   // Hiragana, Katakana, CJK symbols, compatibility
        0x3400..=0x4DBF |   // CJK Extension A
        0x4E00..=0x9FFF |   // CJK Unified Ideographs
        0xA000..=0xA4CF |   // Yi
        0xAC00..=0xD7A3 |   // Hangul Syllables
        0xF900..=0xFAFF |   // CJK Compatibility Ideographs
        0xFE30..=0xFE4F |   // CJK Compatibility Forms
        0xFF00..=0xFF60 |   // Full-width forms
        0xFFE0..=0xFFE6 |   // Full-width signs
        0x1F300..=0x1FAFF | // emoji / pictographs
        0x20000..=0x3FFFD   // CJK Extension B and beyond
    );
    if wide {
        2.0
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_scales_with_font_size() {
        // At the base font the width is exactly chars × base_char_w.
        assert_eq!(text_width("hello", 7.5, BASE_FONT_SIZE), 5.0 * 7.5);
        // Doubling the font size doubles the estimated width.
        assert_eq!(
            text_width("hello", 7.5, 2.0 * BASE_FONT_SIZE),
            2.0 * text_width("hello", 7.5, BASE_FONT_SIZE)
        );
    }

    #[test]
    fn scale_is_identity_at_base() {
        assert_eq!(font_scale(BASE_FONT_SIZE), 1.0);
    }

    #[test]
    fn cjk_glyphs_count_double() {
        // #187: East-Asian-wide chars are ~1em, counted as two half-width units.
        assert_eq!(text_width("中文", 7.5, BASE_FONT_SIZE), 4.0 * 7.5);
        // Mixed ASCII + CJK: 2 half-widths + 2 wide = 2 + 4 = 6 units.
        assert_eq!(text_width("ab中文", 7.5, BASE_FONT_SIZE), 6.0 * 7.5);
        // Pure ASCII is unchanged, so the whole gallery stays byte-identical.
        assert_eq!(text_width("hello", 7.5, BASE_FONT_SIZE), 5.0 * 7.5);
    }
}
