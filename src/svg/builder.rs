//! Minimal SVG builder. Concatenates element strings into a buffer.
//!
//! We do not depend on quick-xml: SVG output is write-only, escaping is
//! cheap, and a string builder keeps the dependency tree small.

use std::borrow::Cow;
use std::fmt::Write as _;

use super::markup::{parse_spans, Span};
use super::theme::Theme;

/// Baseline-to-baseline spacing used when a label is split across lines.
pub const LABEL_LINE_H: f64 = 18.0;

pub struct SvgBuilder {
    pub body: String,
    pub defs: String,
    pub width: f64,
    pub height: f64,
    pub font_family: Cow<'static, str>,
    pub font_size: f64,
    /// Emit the responsive `width="100%"` + `max-width` envelope. When `false`
    /// (`config.useMaxWidth: false`) a fixed pixel `width`/`height` is emitted.
    pub responsive: bool,
}

impl SvgBuilder {
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            body: String::new(),
            defs: String::new(),
            width,
            height,
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
        }
    }

    /// Adopt the theme's font and responsiveness in one call. Chainable so call
    /// sites read `SvgBuilder::new(w, h).theme(theme)`.
    pub fn theme(mut self, theme: &Theme) -> Self {
        self.font_family = theme.font_family.clone();
        self.font_size = theme.font_size;
        self.responsive = theme.responsive;
        self
    }

    pub fn finish(self) -> String {
        let mut out = String::with_capacity(self.body.len() + self.defs.len() + 256);
        // Responsive envelope, matching upstream Mermaid: a fluid `width="100%"`
        // capped by `max-width`, with the intrinsic size carried by `viewBox`
        // (no fixed `height`, so the aspect ratio is preserved when scaled).
        // `config.useMaxWidth: false` instead pins a fixed pixel `width`/`height`.
        let sizing = if self.responsive {
            format!(
                "width=\"100%\" viewBox=\"0 0 {w} {h}\" style=\"max-width: {w}px;\"",
                w = fnum(self.width),
                h = fnum(self.height),
            )
        } else {
            format!(
                "width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"",
                w = fnum(self.width),
                h = fnum(self.height),
            )
        };
        let _ = write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" \
             {sizing} font-family=\"{ff}\" font-size=\"{fs}\">",
            ff = escape(&self.font_family),
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
        let lines = split_label_lines(content);
        let parsed: Vec<Vec<Span>> = lines.iter().map(|l| parse_spans(l)).collect();
        // Fast path: a single line of plain text stays a bare <text>, so
        // tag-free labels render byte-identically to before inline HTML support.
        if parsed.len() == 1 && parsed[0].len() == 1 && parsed[0][0].is_plain() {
            let _ = write!(
                self.body,
                "<text x=\"{}\" y=\"{}\" {}>{}</text>",
                fnum(x),
                fnum(y),
                attrs,
                escape(&parsed[0][0].text)
            );
            return;
        }
        // Multi-line and/or inline-HTML label: stack the lines as <tspan>s
        // centered vertically on the baseline `y` (so <br>/\n break lines) and
        // emit one styled <tspan> per run within a line (so <b>/<i>/<u>/<span>
        // style inline, <a href> wraps a link). Line spacing tracks the font
        // size so stacked lines don't crowd/overlap once `--font-size` grows.
        let line_h = LABEL_LINE_H * super::metrics::font_scale(self.font_size);
        let first_dy = -((lines.len() as f64 - 1.0) * line_h) / 2.0;
        let mut spans = String::new();
        for (li, line_spans) in parsed.iter().enumerate() {
            for (si, span) in line_spans.iter().enumerate() {
                // Only the first run of a line carries the x/dy that positions
                // the line; later runs flow inline after it.
                let pos = if si == 0 {
                    let dy = if li == 0 { first_dy } else { line_h };
                    format!(" x=\"{}\" dy=\"{}\"", fnum(x), fnum(dy))
                } else {
                    String::new()
                };
                let mut style = String::new();
                if span.bold {
                    style.push_str(" font-weight=\"bold\"");
                }
                if span.italic {
                    style.push_str(" font-style=\"italic\"");
                }
                if span.underline {
                    style.push_str(" text-decoration=\"underline\"");
                }
                if let Some(color) = &span.color {
                    let _ = write!(style, " fill=\"{}\"", escape(color));
                }
                let tspan = format!("<tspan{pos}{style}>{}</tspan>", escape(&span.text));
                match &span.href {
                    Some(href) => {
                        let _ = write!(spans, "<a href=\"{}\">{tspan}</a>", escape(href));
                    }
                    None => spans.push_str(&tspan),
                }
            }
        }
        let _ = write!(
            self.body,
            "<text x=\"{}\" y=\"{}\" {}>{}</text>",
            fnum(x),
            fnum(y),
            attrs,
            spans
        );
    }

    pub fn defs_raw(&mut self, raw: &str) {
        self.defs.push_str(raw);
    }

    pub fn raw(&mut self, raw: &str) {
        self.body.push_str(raw);
    }
}

/// Split a label into display lines, honoring the line breaks that upstream
/// Mermaid recognizes: HTML `<br>` / `<br/>` / `<br />` (case-insensitive, with
/// optional inner whitespace) and `\n` — the latter as a real newline or the
/// two-character literal escape `\n` that survives in Mermaid source. Each line
/// is trimmed of surrounding whitespace. A label with no breaks yields a single
/// element, so callers keep their existing single-line behavior.
pub fn split_label_lines(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut lines: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\n' {
            lines.push(s[start..i].trim());
            i += 1;
            start = i;
        } else if b[i] == b'\\' && i + 1 < b.len() && b[i + 1] == b'n' {
            lines.push(s[start..i].trim());
            i += 2;
            start = i;
        } else if let Some(len) = match_br(&b[i..]) {
            lines.push(s[start..i].trim());
            i += len;
            start = i;
        } else {
            i += 1;
        }
    }
    lines.push(s[start..].trim());
    lines
}

/// Length of a `<br>`-style tag at the start of `b`, or `None`. Matches `<br`
/// then optional whitespace, an optional `/`, more optional whitespace, and `>`.
fn match_br(b: &[u8]) -> Option<usize> {
    if b.len() < 4 || b[0] != b'<' {
        return None;
    }
    if !b[1].eq_ignore_ascii_case(&b'b') || !b[2].eq_ignore_ascii_case(&b'r') {
        return None;
    }
    let mut j = 3;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j < b.len() && b[j] == b'/' {
        j += 1;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
    }
    if j < b.len() && b[j] == b'>' {
        Some(j + 1)
    } else {
        None
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
    fn envelope_is_responsive() {
        let svg = SvgBuilder::new(120.0, 80.0).finish();
        assert!(svg.contains("width=\"100%\""));
        assert!(svg.contains("style=\"max-width: 120px;\""));
        assert!(svg.contains("viewBox=\"0 0 120 80\""));
        // No fixed pixel height on the root element.
        assert!(!svg.contains("height=\""));
    }

    #[test]
    fn text_decodes_entities_into_content() {
        let mut b = SvgBuilder::new(50.0, 20.0);
        b.text(10.0, 10.0, "", "x #gt; y");
        let svg = b.finish();
        // `#gt;` → `>` → XML-escaped back to `&gt;` (never leaks the raw `#gt;`).
        assert!(svg.contains("x &gt; y"));
        assert!(!svg.contains("#gt;"));
    }

    #[test]
    fn applies_custom_font() {
        let theme = Theme::default_theme()
            .with_font("Inter, sans-serif")
            .with_font_size(16.0);
        let svg = SvgBuilder::new(10.0, 10.0).theme(&theme).finish();
        assert!(svg.contains("font-family=\"Inter, sans-serif\""));
        assert!(svg.contains("font-size=\"16\""));
    }

    #[test]
    fn non_responsive_emits_fixed_size() {
        let mut b = SvgBuilder::new(120.0, 80.0);
        b.responsive = false;
        let svg = b.finish();
        assert!(svg.contains("width=\"120\""));
        assert!(svg.contains("height=\"80\""));
        assert!(!svg.contains("max-width"));
        assert!(!svg.contains("width=\"100%\""));
    }

    #[test]
    fn splits_labels_on_br_and_newlines() {
        assert_eq!(split_label_lines("one line"), vec!["one line"]);
        assert_eq!(split_label_lines("a<br>b"), vec!["a", "b"]);
        assert_eq!(split_label_lines("a<br/>b<br />c"), vec!["a", "b", "c"]);
        assert_eq!(split_label_lines("a<BR/>b"), vec!["a", "b"]);
        assert_eq!(split_label_lines("a<br  / >b"), vec!["a", "b"]);
        // Real newline and the two-char literal escape both split.
        assert_eq!(split_label_lines("a\nb"), vec!["a", "b"]);
        assert_eq!(split_label_lines("a\\nb"), vec!["a", "b"]);
        // Each line is trimmed of surrounding whitespace.
        assert_eq!(split_label_lines("a <br/> b"), vec!["a", "b"]);
    }

    #[test]
    fn br_not_matched_inside_words() {
        assert_eq!(split_label_lines("abrupt"), vec!["abrupt"]);
        assert_eq!(split_label_lines("<break>"), vec!["<break>"]);
    }

    #[test]
    fn multiline_text_emits_tspans_no_literal_br() {
        let mut b = SvgBuilder::new(200.0, 60.0);
        b.text(
            50.0,
            30.0,
            "text-anchor=\"middle\"",
            "line1<br/>line2<br/>line3",
        );
        let svg = b.finish();
        assert_eq!(svg.matches("<tspan").count(), 3);
        assert!(svg.contains(">line1</tspan>"));
        assert!(svg.contains(">line3</tspan>"));
        assert!(!svg.contains("br/"));
        assert!(!svg.contains("&lt;br"));
    }

    #[test]
    fn inline_html_styles_render_as_tspans() {
        let mut b = SvgBuilder::new(200.0, 40.0);
        b.text(
            50.0,
            20.0,
            "text-anchor=\"middle\"",
            "<b>bold</b> <i>it</i> <u>u</u>",
        );
        let svg = b.finish();
        assert!(svg.contains("font-weight=\"bold\">bold</tspan>"));
        assert!(svg.contains("font-style=\"italic\">it</tspan>"));
        assert!(svg.contains("text-decoration=\"underline\">u</tspan>"));
        // The tags themselves never leak as literal text.
        assert!(!svg.contains("&lt;b&gt;"));
    }

    #[test]
    fn inline_html_color_span_and_link() {
        let mut b = SvgBuilder::new(200.0, 40.0);
        b.text(
            50.0,
            20.0,
            "",
            "<span style=\"color:red\">r</span><a href=\"http://x\">y</a>",
        );
        let svg = b.finish();
        assert!(svg.contains("fill=\"red\">r</tspan>"));
        assert!(svg.contains("<a href=\"http://x\"><tspan"));
        assert!(svg.contains(">y</tspan></a>"));
    }

    #[test]
    fn unknown_tags_strip_to_plain_text() {
        let mut b = SvgBuilder::new(200.0, 40.0);
        b.text(50.0, 20.0, "", "a<div>b</div>c");
        let svg = b.finish();
        // Stripped, merged, and kept on the single-line fast path (no tspans).
        assert!(svg.contains(">abc</text>"));
        assert!(!svg.contains("<tspan"));
        assert!(!svg.contains("div"));
    }

    #[test]
    fn plain_single_line_stays_bare_text() {
        let mut b = SvgBuilder::new(200.0, 40.0);
        b.text(50.0, 20.0, "", "just text");
        let svg = b.finish();
        assert!(svg.contains(">just text</text>"));
        assert!(!svg.contains("<tspan"));
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
