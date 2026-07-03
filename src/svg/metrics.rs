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
pub(crate) fn text_width(s: &str, base_char_w: f64, font_size: f64) -> f64 {
    s.chars().count() as f64 * base_char_w * font_scale(font_size)
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
}
