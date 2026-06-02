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

/// Faithful port of d3-shape `curveBasis` (open cubic B-spline) to an SVG path.
///
/// Endpoints are exact: the path starts at `pts[0]` and ends at `pts[n-1]`, so
/// node-boundary clipping and arrow markers still line up.
///   `0` pts → `""`            `1` pt → `"M x y"`
///   `2` pts → `"M x0 y0L x1 y1"` (straight; short edges stay straight)
///   `≥3`    → moveTo(p0); lineTo((5p0+p1)/6); a bezier per interior point;
///             a closing bezier; a final lineTo(last point).
pub(crate) fn curve_basis_path(pts: &[(f64, f64)]) -> String {
    let n = pts.len();
    let mut s = String::new();
    if n == 0 {
        return s;
    }
    if n == 1 {
        let _ = write!(s, "M{} {}", fnum(pts[0].0), fnum(pts[0].1));
        return s;
    }
    if n == 2 {
        let _ = write!(
            s,
            "M{} {}L{} {}",
            fnum(pts[0].0),
            fnum(pts[0].1),
            fnum(pts[1].0),
            fnum(pts[1].1),
        );
        return s;
    }

    fn bezier(s: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
        let _ = write!(
            s,
            "C{} {} {} {} {} {}",
            fnum((2.0 * x0 + x1) / 3.0),
            fnum((2.0 * y0 + y1) / 3.0),
            fnum((x0 + 2.0 * x1) / 3.0),
            fnum((y0 + 2.0 * y1) / 3.0),
            fnum((x0 + 4.0 * x1 + x) / 6.0),
            fnum((y0 + 4.0 * y1 + y) / 6.0),
        );
    }

    let (mut x0, mut y0) = (f64::NAN, f64::NAN);
    let (mut x1, mut y1) = (f64::NAN, f64::NAN);

    for (i, &(x, y)) in pts.iter().enumerate() {
        match i {
            0 => {
                let _ = write!(s, "M{} {}", fnum(x), fnum(y));
            }
            1 => { /* d3 stores the point only; emits nothing */ }
            2 => {
                let _ = write!(
                    s,
                    "L{} {}",
                    fnum((5.0 * x0 + x1) / 6.0),
                    fnum((5.0 * y0 + y1) / 6.0)
                );
                bezier(&mut s, x0, y0, x1, y1, x, y);
            }
            _ => bezier(&mut s, x0, y0, x1, y1, x, y),
        }
        x0 = x1;
        y0 = y1;
        x1 = x;
        y1 = y;
    }

    // d3 Basis.lineEnd for an open curve with ≥3 points: one final bezier using
    // the incoming point = the last point, then a lineTo to the exact last
    // point. After the loop (x1,y1)=last and (x0,y0)=second-to-last.
    bezier(&mut s, x0, y0, x1, y1, x1, y1);
    let _ = write!(s, "L{} {}", fnum(x1), fnum(y1));

    s
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

    #[test]
    fn curve_basis_degenerate() {
        assert_eq!(curve_basis_path(&[]), "");
        assert_eq!(curve_basis_path(&[(3.0, 4.0)]), "M3 4");
    }

    #[test]
    fn curve_basis_two_points_is_straight() {
        let d = curve_basis_path(&[(0.0, 0.0), (10.0, 0.0)]);
        assert_eq!(d, "M0 0L10 0");
        assert!(!d.contains('C'));
    }

    #[test]
    fn curve_basis_three_points_curves_with_exact_endpoints() {
        let pts = [(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)];
        let d = curve_basis_path(&pts);
        assert!(d.starts_with("M0 0"));
        assert!(d.contains('C'));
        // Endpoint exactness: path must end at the last point.
        assert!(d.ends_with("L20 0"));
    }
}
