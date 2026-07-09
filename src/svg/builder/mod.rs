//! Minimal SVG builder. Concatenates element strings into a buffer.
//!
//! We do not depend on quick-xml: SVG output is write-only, escaping is
//! cheap, and a string builder keeps the dependency tree small.

use std::borrow::Cow;
use std::fmt::Write as _;

use super::markup::{parse_lines, Span};
use super::theme::Theme;

mod curves;
#[cfg(test)]
mod tests;
mod text;

pub(crate) use curves::{curve_basis_path, curve_linear_path, curve_step_path};
pub use text::{escape, split_label_lines};

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
        // Parse all lines together so an inline tag opened before a `<br>`
        // still styles the text after it (#187).
        let parsed: Vec<Vec<Span>> = parse_lines(&lines);
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

    /// Define a triangular arrowhead `<marker>`: a filled `M0,0 L10,5 L0,10 z`
    /// triangle in a `0 0 10 10` viewBox, oriented `auto-start-reverse` so the
    /// same id serves both `marker-start` and `marker-end`. `ref_x` places the
    /// tip on the node boundary; `size` sets both marker dimensions.
    pub fn def_arrow_marker(&mut self, id: &str, color: &str, ref_x: u32, size: u32) {
        let _ = write!(
            self.defs,
            "<marker id=\"{id}\" viewBox=\"0 0 10 10\" refX=\"{ref_x}\" refY=\"5\" \
             markerWidth=\"{size}\" markerHeight=\"{size}\" orient=\"auto-start-reverse\">\
             <path d=\"M0,0 L10,5 L0,10 z\" fill=\"{color}\"/></marker>"
        );
    }

    pub fn raw(&mut self, raw: &str) {
        self.body.push_str(raw);
    }
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
