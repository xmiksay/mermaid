//! Minimal SVG builder. Concatenates element strings into a buffer.
//!
//! We do not depend on quick-xml: SVG output is write-only, escaping is
//! cheap, and a string builder keeps the dependency tree small.

use std::fmt::Write as _;

pub struct SvgBuilder {
    pub body: String,
    pub defs: String,
    pub width: f64,
    pub height: f64,
    pub font_family: &'static str,
    pub font_size: f64,
}

impl SvgBuilder {
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            body: String::new(),
            defs: String::new(),
            width,
            height,
            font_family: "sans-serif",
            font_size: 14.0,
        }
    }

    /// Set the root `font-family`/`font-size` from a theme. Chainable so call
    /// sites read `SvgBuilder::new(w, h).font(theme.font_family, theme.font_size)`.
    pub fn font(mut self, family: &'static str, size: f64) -> Self {
        self.font_family = family;
        self.font_size = size;
        self
    }

    pub fn finish(self) -> String {
        let mut out = String::with_capacity(self.body.len() + self.defs.len() + 256);
        let _ = write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" \
             width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" \
             font-family=\"{ff}\" font-size=\"{fs}\">",
            w = fnum(self.width),
            h = fnum(self.height),
            ff = escape(self.font_family),
            fs = fnum(self.font_size),
        );
        if !self.defs.is_empty() {
            let _ = write!(out, "<defs>{}</defs>", self.defs);
        }
        out.push_str(&self.body);
        out.push_str("</svg>");
        out
    }

    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, attrs: &str) {
        let _ = write!(
            self.body,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {}/>",
            fnum(x),
            fnum(y),
            fnum(w),
            fnum(h),
            attrs
        );
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, attrs: &str) {
        let _ = write!(
            self.body,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {}/>",
            fnum(x1),
            fnum(y1),
            fnum(x2),
            fnum(y2),
            attrs
        );
    }

    pub fn path(&mut self, d: &str, attrs: &str) {
        let _ = write!(self.body, "<path d=\"{d}\" {attrs}/>");
    }

    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, attrs: &str) {
        let _ = write!(
            self.body,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {}/>",
            fnum(cx),
            fnum(cy),
            fnum(r),
            attrs
        );
    }

    pub fn text(&mut self, x: f64, y: f64, attrs: &str, content: &str) {
        let _ = write!(
            self.body,
            "<text x=\"{}\" y=\"{}\" {}>{}</text>",
            fnum(x),
            fnum(y),
            attrs,
            escape(content)
        );
    }

    pub fn defs_raw(&mut self, raw: &str) {
        self.defs.push_str(raw);
    }

    pub fn raw(&mut self, raw: &str) {
        self.body.push_str(raw);
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Format an f64 with up to 3 decimals, trimming trailing zeros so the SVG
/// stays compact and matches across platforms.
pub fn fnum(v: f64) -> String {
    if v.fract().abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_specials() {
        assert_eq!(escape("a < b & c"), "a &lt; b &amp; c");
        assert_eq!(escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(fnum(1.0), "1");
        assert_eq!(fnum(1.5), "1.5");
        assert_eq!(fnum(1.123456), "1.123");
        assert_eq!(fnum(-2.0), "-2");
    }

    #[test]
    fn builds_valid_svg_envelope() {
        let mut b = SvgBuilder::new(100.0, 50.0);
        b.rect(0.0, 0.0, 100.0, 50.0, "fill=\"red\"");
        let svg = b.finish();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("viewBox=\"0 0 100 50\""));
        assert!(svg.contains("fill=\"red\""));
        assert!(svg.contains("font-family=\"sans-serif\""));
        assert!(svg.contains("font-size=\"14\""));
    }

    #[test]
    fn applies_custom_font() {
        let svg = SvgBuilder::new(10.0, 10.0)
            .font("Inter, sans-serif", 16.0)
            .finish();
        assert!(svg.contains("font-family=\"Inter, sans-serif\""));
        assert!(svg.contains("font-size=\"16\""));
    }
}
