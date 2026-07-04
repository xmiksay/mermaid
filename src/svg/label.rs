//! Label text decoding shared by every renderer, applied by
//! [`SvgBuilder::text`][super::builder::SvgBuilder::text] before XML escaping.
//!
//! Ports the pre-render text handling upstream Mermaid does: resolving
//! `#`-prefixed entity codes. Backtick-fenced markdown *strings* and their
//! `**bold**`/`*italic*` emphasis are handled one layer up, in
//! [`markup::parse_lines`][super::markup::parse_lines], which emits styled
//! `<tspan>`s rather than flattening the emphasis to plain text.

use super::builder::SvgBuilder;
use super::theme::Theme;

/// Draw an opaque background rect — upstream Mermaid's `edgeLabelBackground`
/// treatment — sized `width`×`height` and centered on `(cx, cy)`, so an edge
/// label drawn on top stays legible where the edge crosses a node or another
/// label (#260). The fill is the theme's `flow_label_bg` (wired from the
/// `edgeLabelBackground` theme variable).
pub(crate) fn edge_label_bg(
    svg: &mut SvgBuilder,
    cx: f64,
    cy: f64,
    width: f64,
    height: f64,
    theme: &Theme,
) {
    svg.rect(
        cx - width / 2.0,
        cy - height / 2.0,
        width,
        height,
        &format!("fill=\"{}\" stroke=\"none\"", theme.flow_label_bg),
    );
}

/// Draw a centered single-line edge label on an opaque [`edge_label_bg`],
/// matching upstream's edge-label styling (12px, theme foreground). Shared by
/// the graph-shaped renderers (flowchart, state, class, ER, block).
pub(crate) fn draw_edge_label(
    svg: &mut SvgBuilder,
    (mx, my): (f64, f64),
    text: &str,
    theme: &Theme,
) {
    let w = super::metrics::text_width(text, 7.0, theme.font_size) + 8.0;
    edge_label_bg(svg, mx, my, w, 18.0, theme);
    svg.text(
        mx,
        my + 4.0,
        &format!(
            "text-anchor=\"middle\" fill=\"{}\" font-size=\"12\"",
            theme.fg
        ),
        text,
    );
}

/// Decode a display label the way upstream Mermaid does before rendering text:
/// strip KaTeX math fences, then resolve `#`-prefixed entity codes.
///
/// Mermaid uses `#` (not `&`) as the entity sentinel in diagram source, so
/// `#quot;` → `"`, `#35;` → `#`, `#9829;`/`#x2665;` → `♥`.
pub fn decode_label(s: &str) -> String {
    decode_entities(&strip_math(s))
}

/// Strip KaTeX `$$…$$` math fences, keeping the inner expression as plain text.
/// Full KaTeX layout is out of scope for a static renderer, so degrading
/// `$$x^2$$` to `x^2` reads better than leaking the raw delimiters (#187). Only
/// matched `$$` pairs are unwrapped; a lone `$$` is left untouched.
fn strip_math(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains("$$") {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("$$") {
        match rest[start + 2..].find("$$") {
            Some(end_rel) => {
                out.push_str(&rest[..start]);
                out.push_str(rest[start + 2..start + 2 + end_rel].trim());
                rest = &rest[start + 2 + end_rel + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

fn decode_entities(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            if let Some((decoded, len)) = decode_entity(&s[i..]) {
                out.push_str(&decoded);
                i += len;
                continue;
            }
        }
        // Push one UTF-8 char, not one byte, to stay valid on multibyte input.
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Decode a single `#…;` entity at the start of `s`, returning `(text, len)`.
fn decode_entity(s: &str) -> Option<(String, usize)> {
    let body = s.strip_prefix('#')?;
    let end = body.find(';')?;
    let token = &body[..end];
    if token.is_empty() || token.len() > 10 {
        return None;
    }
    let len = 1 + end + 1; // '#' + token + ';'
    let ch = if let Some(hex) = token.strip_prefix(['x', 'X']) {
        char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
    } else if token.bytes().all(|c| c.is_ascii_digit()) {
        char::from_u32(token.parse().ok()?)?
    } else {
        match token {
            "quot" => '"',
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "apos" => '\'',
            "nbsp" => '\u{a0}',
            "hearts" => '♥',
            "colon" => ':',
            "semi" => ';',
            _ => return None,
        }
    };
    Some((ch.to_string(), len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_label_draws_background_before_text() {
        let theme = Theme::default();
        let mut svg = SvgBuilder::new(100.0, 100.0);
        draw_edge_label(&mut svg, (50.0, 20.0), "yes", &theme);
        let out = svg.finish();
        // The opaque rect (edgeLabelBackground) must precede the label text so
        // it sits behind it (#260).
        let rect = out.find(&format!("fill=\"{}\" stroke=\"none\"", theme.flow_label_bg));
        let text = out.find(">yes<");
        assert!(rect.is_some(), "expected a background rect");
        assert!(rect < text, "background must come before the label text");
    }

    #[test]
    fn decodes_entity_codes() {
        assert_eq!(decode_label("a #quot;b#quot; c"), "a \"b\" c");
        assert_eq!(decode_label("#35; hash"), "# hash");
        assert_eq!(decode_label("#9829; and #x2665;"), "♥ and ♥");
        assert_eq!(decode_label("a #lt; b #gt; c"), "a < b > c");
        // Not an entity: left untouched.
        assert_eq!(decode_label("C#Sharp"), "C#Sharp");
        assert_eq!(decode_label("#notclosed"), "#notclosed");
    }

    #[test]
    fn strips_math_fences() {
        // #187: `$$…$$` KaTeX fences degrade to their inner expression.
        assert_eq!(decode_label("$$x^2$$"), "x^2");
        assert_eq!(decode_label("a $$x + y$$ b"), "a x + y b");
        // A lone/unmatched `$$` is left untouched, as is bare currency.
        assert_eq!(decode_label("cost is $$"), "cost is $$");
        assert_eq!(decode_label("$5 and $10"), "$5 and $10");
    }

    #[test]
    fn leaves_markdown_and_backticks_to_the_span_layer() {
        // decode_label no longer touches markdown fences/markers — that is the
        // span layer's job (markup::parse_spans). Bare `_`/`*` are never mangled.
        assert_eq!(decode_label("snake_case"), "snake_case");
        assert_eq!(decode_label("a * b"), "a * b");
    }
}
