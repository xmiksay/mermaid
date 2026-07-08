//! Shared color helpers used across the diagram renderers.

/// Readable text color for a label drawn on a `hex` fill — dark on light
/// fills, white on dark ones. Many built-in palettes (the default `cScale`
/// pastels, the git lane colors) are light, so most read dark; hard-coding
/// white text on them makes labels invisible (issue #314). Non-hex colors
/// fall back to dark text.
pub(super) fn readable_text_color(hex: &str) -> &'static str {
    let h = hex.trim_start_matches('#');
    if h.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            return if lum > 140.0 { "#333333" } else { "#ffffff" };
        }
    }
    "#333333"
}

#[cfg(test)]
mod tests {
    use super::readable_text_color;

    #[test]
    fn dark_text_on_pale_fills() {
        // Default cScale pastels used by journey sections / kanban columns.
        assert_eq!(readable_text_color("#B9B9FF"), "#333333");
        assert_eq!(readable_text_color("#FFFFAB"), "#333333");
        assert_eq!(readable_text_color("#E8FFB9"), "#333333");
    }

    #[test]
    fn white_text_on_dark_fills() {
        assert_eq!(readable_text_color("#000000"), "#ffffff");
        assert_eq!(readable_text_color("#2b2b2b"), "#ffffff");
    }

    #[test]
    fn non_hex_falls_back_to_dark() {
        assert_eq!(readable_text_color("red"), "#333333");
        assert_eq!(readable_text_color("#abc"), "#333333");
    }
}
