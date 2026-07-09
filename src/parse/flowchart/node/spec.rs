//! Flowchart node-spec scanning.
//!
//! Reads a single node group into `FlowNode`s: the shape brackets (`[]`, `()`,
//! `([ ])`, `[[]]`, `[( )]`, `(())`, `((()))`, `{}`, `{{}}`, the
//! parallelogram/trapezoid family, and the `> ]` flag), the Mermaid v11
//! `id@{ shape: …, label: … }` attribute block, the `&`-joined multi-target
//! groups, and the `:::class` shorthand.

use crate::parse::ast::{FlowNode, NodeShape, Style};
use crate::parse::token::{find_unquoted, parse_attr_pairs, unquote};
use crate::parse::ParseError;

use super::super::scanner::Scanner;
use super::shapes::shape_from_name;

pub(super) fn parse_node_group(
    sc: &mut Scanner<'_>,
    line_no: usize,
) -> Result<Vec<FlowNode>, ParseError> {
    let mut out = Vec::new();
    loop {
        sc.skip_ws();
        let node = parse_node_spec(sc, line_no)?;
        out.push(node);
        sc.skip_ws();
        if !sc.try_consume("&") {
            break;
        }
    }
    Ok(out)
}

fn parse_node_spec(sc: &mut Scanner<'_>, line_no: usize) -> Result<FlowNode, ParseError> {
    sc.skip_ws();
    // The asymmetric `>` flag opens BEFORE an id appears, but Mermaid actually
    // requires an id first. The shape opener is detected after the id.
    let id = sc.read_ident().ok_or_else(|| {
        ParseError::malformed(
            line_no,
            format!("expected node identifier at: '{}'", sc.remaining()),
        )
    })?;

    // Mermaid v11 attribute syntax: `id@{ shape: …, label: … }`.
    if sc.peek_str("@{") {
        return parse_at_node(id, sc, line_no);
    }

    // The shape table: longer openers first so they win over their prefixes.
    const SHAPES: &[(&str, &str, NodeShape)] = &[
        ("(((", ")))", NodeShape::DoubleCircle),
        ("([", "])", NodeShape::Stadium),
        ("[[", "]]", NodeShape::Subroutine),
        ("[(", ")]", NodeShape::Cylinder),
        ("((", "))", NodeShape::Circle),
        ("{{", "}}", NodeShape::Hexagon),
        ("[/", "/]", NodeShape::Parallelogram),
        ("[\\", "\\]", NodeShape::ParallelogramAlt),
        // trapezoids — must be tried before plain `[`/`/]`
        ("[/", "\\]", NodeShape::Trapezoid),
        ("[\\", "/]", NodeShape::TrapezoidAlt),
        (">", "]", NodeShape::Asymmetric),
        ("[", "]", NodeShape::Rect),
        ("(", ")", NodeShape::Round),
        ("{", "}", NodeShape::Rhombus),
    ];
    // For each opener, try matching with its specific closer. Multi-closer shapes
    // (parallelogram vs trapezoid) share the `[/` opener — so when we see `[/`
    // we scan until the first matching closer of either `/]` or `\]`.
    if sc.peek_str("[/") || sc.peek_str("[\\") {
        sc.advance(2);
        let opener_was_slash = sc.s.as_bytes()[sc.i - 1] == b'/';
        // Scan text until we hit `/]` or `\]`.
        let (text, used_close) = read_until_either(sc, "/]", "\\]").ok_or_else(|| {
            ParseError::unclosed(line_no, format!("missing closing for node '{id}'"))
        })?;
        let shape = match (opener_was_slash, used_close) {
            (true, "/]") => NodeShape::Parallelogram,
            (true, "\\]") => NodeShape::Trapezoid,
            (false, "\\]") => NodeShape::ParallelogramAlt,
            (false, "/]") => NodeShape::TrapezoidAlt,
            _ => NodeShape::Rect,
        };
        return Ok(finish_node(id, unquote(text.trim()).to_string(), shape, sc));
    }
    for (open, close, shape) in SHAPES {
        if sc.try_consume(open) {
            let text = sc.read_until_unquoted(close).ok_or_else(|| {
                ParseError::unclosed(
                    line_no,
                    format!("missing closing '{close}' for node '{id}'"),
                )
            })?;
            let _ = sc.try_consume(close);
            return Ok(finish_node(
                id,
                unquote(text.trim()).to_string(),
                *shape,
                sc,
            ));
        }
    }
    let text = id.clone();
    Ok(finish_node(id, text, NodeShape::Rect, sc))
}

/// Parse the v11 `id@{ key: value, … }` attribute block. `shape` maps a named
/// shape onto a `NodeShape` (unknown names fall back to `Rect`, matching
/// upstream); `label`/`title` set the node text. `icon`/`img` forms are out of
/// scope — dropped, but any `label` is still honored so content is never lost.
fn parse_at_node(id: String, sc: &mut Scanner<'_>, line_no: usize) -> Result<FlowNode, ParseError> {
    sc.advance(2); // consume `@{`
    let body = sc.read_until_unquoted("}").ok_or_else(|| {
        ParseError::unclosed(line_no, format!("missing closing '}}' for node '{id}'"))
    })?;
    sc.try_consume("}");

    let mut text = id.clone();
    let mut shape = NodeShape::Rect;
    for (key, value) in parse_attr_pairs(&body) {
        match key.as_str() {
            "shape" => shape = shape_from_name(&value),
            "label" | "title" => text = value,
            _ => {} // icon/img and any other keys are dropped
        }
    }
    Ok(finish_node(id, text, shape, sc))
}

/// Build a node, consuming an optional `:::class` shorthand that follows the
/// id/shape (no whitespace allowed before `:::`, per Mermaid).
fn finish_node(id: String, text: String, shape: NodeShape, sc: &mut Scanner<'_>) -> FlowNode {
    let mut classes = Vec::new();
    if sc.try_consume(":::") {
        if let Some(name) = sc.read_ident() {
            classes.push(name);
        }
    }
    FlowNode {
        id,
        text,
        shape,
        classes,
        style: Style::new(),
        click: None,
    }
}

fn read_until_either<'a>(
    sc: &mut Scanner<'a>,
    a: &'static str,
    b: &'static str,
) -> Option<(String, &'static str)> {
    let rem = sc.remaining();
    let pa = find_unquoted(rem, a);
    let pb = find_unquoted(rem, b);
    let (pos, tok) = match (pa, pb) {
        (Some(x), Some(y)) => {
            if x <= y {
                (x, a)
            } else {
                (y, b)
            }
        }
        (Some(x), None) => (x, a),
        (None, Some(y)) => (y, b),
        (None, None) => return None,
    };
    let text = rem[..pos].to_string();
    sc.i += pos + tok.len();
    Some((text, tok))
}
