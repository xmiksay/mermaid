//! block-beta parser. Grid blocks with optional groups and inline edges.
//!
//! Grammar:
//!
//! ```text
//! block-beta
//!     columns 3
//!     a b c
//!     d["wide"]:2 e
//!     block:group1
//!       h i
//!     end
//!     a --> b
//! ```

use std::collections::HashMap;

use super::ast::{
    Block, BlockArrow, BlockDiagram, BlockEdge, BlockGroup, BlockItem, BlockShape, Style,
};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

/// Style state gathered while scanning: `classDef` definitions plus the
/// deferred `class`/`style` assignments that target block ids (which may be
/// declared before *or* after the assignment line).
#[derive(Default)]
struct Ctx {
    class_defs: HashMap<String, Style>,
    class_assign: Vec<(Vec<String>, String)>,
    style_assign: Vec<(String, Style)>,
}

pub(crate) fn parse(input: &str) -> Result<BlockDiagram, ParseError> {
    let mut d = BlockDiagram::default();
    let mut ctx = Ctx::default();
    let mut header_seen = false;
    let lines: Vec<(usize, String)> = input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, strip_comment(l).to_string()))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();

    let mut i = 0;
    while i < lines.len() {
        let (line_no, line) = (lines[i].0, lines[i].1.trim().to_string());
        i += 1;

        if !header_seen {
            if line != "block-beta" && line != "block" {
                return Err(ParseError::Syntax {
                    message: "expected 'block-beta' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("columns") {
            let v: usize = rest.trim().parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid columns: '{}'", rest.trim()),
                line: line_no,
            })?;
            d.columns = Some(v);
            continue;
        }

        if handle_style_line(&line, &mut ctx) {
            continue;
        }

        if let Some(rest) = line.strip_prefix("block:") {
            let (id, span) = split_block_head(rest);
            let inner = collect_until_end(&lines, &mut i);
            let group = parse_group(id, span, &inner, &mut ctx)?;
            d.items.push(BlockItem::Group(group));
            continue;
        }
        if line == "block" {
            let inner = collect_until_end(&lines, &mut i);
            let group = parse_group(String::new(), 1, &inner, &mut ctx)?;
            d.items.push(BlockItem::Group(group));
            continue;
        }

        if let Some(edge) = parse_edge(&line) {
            d.items.push(BlockItem::Edge(edge));
            continue;
        }

        for it in parse_block_line(&line) {
            d.items.push(it);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    apply_assignments(&mut d.items, &ctx);
    d.class_defs = ctx.class_defs;
    Ok(d)
}

/// `block:id` / `block:id:span` → (id, span).
fn split_block_head(rest: &str) -> (String, usize) {
    let mut parts = rest.trim().splitn(2, ':');
    let id = parts.next().unwrap_or("").trim().to_string();
    let span = parts
        .next()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(1);
    (id, span)
}

/// Consume a `classDef`/`class`/`style` statement into `ctx`; returns whether
/// the line was one of those (so the caller skips node tokenizing).
fn handle_style_line(line: &str, ctx: &mut Ctx) -> bool {
    if let Some(rest) = line.strip_prefix("classDef ") {
        if let Some((names, props)) = rest.trim().split_once(char::is_whitespace) {
            let style = parse_style_props(props);
            for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                ctx.class_defs.insert(name.to_string(), style.clone());
            }
        }
        return true;
    }
    if let Some(rest) = line.strip_prefix("class ") {
        if let Some((ids, name)) = rest.trim().rsplit_once(char::is_whitespace) {
            let ids: Vec<String> = ids
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            let name = name.trim();
            if !ids.is_empty() && !name.is_empty() {
                ctx.class_assign.push((ids, name.to_string()));
            }
        }
        return true;
    }
    if let Some(rest) = line.strip_prefix("style ") {
        if let Some((id, props)) = rest.trim().split_once(char::is_whitespace) {
            ctx.style_assign
                .push((id.trim().to_string(), parse_style_props(props)));
        }
        return true;
    }
    false
}

/// Apply deferred `class`/`style` assignments onto matching blocks (recursing
/// into groups), after the whole diagram has been scanned.
fn apply_assignments(items: &mut [BlockItem], ctx: &Ctx) {
    for it in items {
        match it {
            BlockItem::Block(b) => {
                for (ids, name) in &ctx.class_assign {
                    if ids.iter().any(|i| i == &b.id) && !b.classes.contains(name) {
                        b.classes.push(name.clone());
                    }
                }
                for (id, style) in &ctx.style_assign {
                    if id == &b.id {
                        b.style.extend(style.iter().cloned());
                    }
                }
            }
            BlockItem::Group(g) => apply_assignments(&mut g.items, ctx),
            _ => {}
        }
    }
}

fn collect_until_end(lines: &[(usize, String)], i: &mut usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut depth = 1;
    while *i < lines.len() {
        let (n, raw) = (lines[*i].0, lines[*i].1.clone());
        let t = raw.trim();
        *i += 1;
        if t == "end" {
            depth -= 1;
            if depth == 0 {
                return out;
            }
            out.push((n, raw));
            continue;
        }
        if t == "block" || t.starts_with("block:") {
            depth += 1;
        }
        out.push((n, raw));
    }
    out
}

fn parse_group(
    id: String,
    span: usize,
    body: &[(usize, String)],
    ctx: &mut Ctx,
) -> Result<BlockGroup, ParseError> {
    let mut items: Vec<BlockItem> = Vec::new();
    let mut columns: Option<usize> = None;
    let mut i = 0;
    while i < body.len() {
        let line = body[i].1.trim().to_string();
        i += 1;
        if let Some(rest) = line.strip_prefix("columns") {
            columns = rest.trim().parse().ok();
            continue;
        }
        if handle_style_line(&line, ctx) {
            continue;
        }
        if let Some(rest) = line.strip_prefix("block:") {
            let (nid, nspan) = split_block_head(rest);
            let inner = collect_until_end(body, &mut i);
            items.push(BlockItem::Group(parse_group(nid, nspan, &inner, ctx)?));
            continue;
        }
        if let Some(e) = parse_edge(&line) {
            items.push(BlockItem::Edge(e));
            continue;
        }
        for it in parse_block_line(&line) {
            items.push(it);
        }
    }
    Ok(BlockGroup {
        id,
        label: None,
        columns,
        items,
        span,
    })
}

fn parse_block_line(line: &str) -> Vec<BlockItem> {
    // Multiple blocks per line separated by whitespace, e.g. "a b c"
    // Each block can be `id`, `id["label"]`, `id["label"]:2`, `id(("circ"))`, etc.
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_q = false;
    for c in line.chars() {
        if c == '"' {
            in_q = !in_q;
            cur.push(c);
            continue;
        }
        if !in_q {
            match c {
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => depth -= 1,
                _ => {}
            }
        }
        if c.is_whitespace() && depth == 0 && !in_q {
            if !cur.is_empty() {
                if let Some(it) = parse_one_block(&cur) {
                    out.push(it);
                }
                cur.clear();
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        if let Some(it) = parse_one_block(&cur) {
            out.push(it);
        }
    }
    out
}

fn parse_one_block(tok: &str) -> Option<BlockItem> {
    let tok = tok.trim();
    if tok.is_empty() {
        return None;
    }
    // space (count): `space:2` or just `space`
    if let Some(rest) = tok.strip_prefix("space") {
        let n = rest
            .trim_start_matches(':')
            .trim()
            .parse::<usize>()
            .unwrap_or(1);
        return Some(BlockItem::Space(n));
    }
    // `:::className` shorthand, stripped before span so it can't be parsed as one.
    let (tok, classes) = match tok.split_once(":::") {
        Some((t, c)) => (t.trim(), vec![c.trim().to_string()]),
        None => (tok, Vec::new()),
    };
    // Block arrow: `id<["label"]>(dir)`.
    if let Some((id, arrow, label)) = parse_block_arrow(tok) {
        return Some(BlockItem::Block(Block {
            id,
            label,
            shape: BlockShape::Arrow(arrow),
            span: 1,
            classes,
            style: Style::new(),
        }));
    }
    // id[shape...]:span
    let (head, span) = if let Some((h, s)) = tok.rsplit_once(':') {
        if let Ok(n) = s.parse::<usize>() {
            (h, n)
        } else {
            (tok, 1)
        }
    } else {
        (tok, 1)
    };
    let (id, shape, label) = parse_shape(head);
    Some(BlockItem::Block(Block {
        id,
        label,
        shape,
        span,
        classes,
        style: Style::new(),
    }))
}

/// Parse a block arrow `id<["label"]>(dir[, dir…])`. Returns `(id, arrow, label)`.
fn parse_block_arrow(s: &str) -> Option<(String, BlockArrow, String)> {
    let lt = s.find("<[")?;
    let gt = s[lt..].find("]>").map(|p| lt + p)?;
    let id = s[..lt].trim().to_string();
    let label_raw = s[lt + 2..gt].trim().trim_matches('"').trim().to_string();
    let dirs = s[gt + 2..].trim();
    let inner = dirs.strip_prefix('(')?.strip_suffix(')')?;
    let mut arrow = BlockArrow::default();
    for d in inner.split(',') {
        match d.trim() {
            "right" => arrow.right = true,
            "left" => arrow.left = true,
            "up" => arrow.up = true,
            "down" => arrow.down = true,
            "x" => {
                arrow.left = true;
                arrow.right = true;
            }
            "y" => {
                arrow.up = true;
                arrow.down = true;
            }
            _ => {}
        }
    }
    if !(arrow.right || arrow.left || arrow.up || arrow.down) {
        arrow.right = true;
    }
    let label = if label_raw.is_empty() {
        id.clone()
    } else {
        label_raw
    };
    Some((id, arrow, label))
}

fn parse_shape(s: &str) -> (String, BlockShape, String) {
    let s = s.trim();
    let shape_start = s.find(['[', '(', '{']);
    let (id, shape_part) = match shape_start {
        Some(p) => (&s[..p], &s[p..]),
        None => (s, ""),
    };
    let (shape, label_raw) = if shape_part.is_empty() {
        (BlockShape::Rect, id.to_string())
    } else if shape_part.starts_with("[(") && shape_part.ends_with(")]") {
        (
            BlockShape::Cylinder,
            shape_part[2..shape_part.len() - 2].to_string(),
        )
    } else if shape_part.starts_with("((") && shape_part.ends_with("))") {
        (
            BlockShape::Circle,
            shape_part[2..shape_part.len() - 2].to_string(),
        )
    } else if shape_part.starts_with("([") && shape_part.ends_with("])") {
        (
            BlockShape::Stadium,
            shape_part[2..shape_part.len() - 2].to_string(),
        )
    } else if shape_part.starts_with("{{") && shape_part.ends_with("}}") {
        (
            BlockShape::Hexagon,
            shape_part[2..shape_part.len() - 2].to_string(),
        )
    } else if shape_part.starts_with('[') && shape_part.ends_with(']') {
        (
            BlockShape::Rect,
            shape_part[1..shape_part.len() - 1].to_string(),
        )
    } else if shape_part.starts_with('(') && shape_part.ends_with(')') {
        (
            BlockShape::Round,
            shape_part[1..shape_part.len() - 1].to_string(),
        )
    } else if shape_part.starts_with('{') && shape_part.ends_with('}') {
        (
            BlockShape::Rhombus,
            shape_part[1..shape_part.len() - 1].to_string(),
        )
    } else {
        (BlockShape::Rect, id.to_string())
    };
    let label = label_raw.trim().trim_matches('"').to_string();
    let label = if label.is_empty() {
        id.to_string()
    } else {
        label
    };
    (id.to_string(), shape, label)
}

fn parse_edge(line: &str) -> Option<BlockEdge> {
    // Match: a --> b, a -- "label" --> b, a --- b
    for arrow in ["-->", "---"] {
        if let Some(pos) = line.find(arrow) {
            let mut from = line[..pos].trim().to_string();
            let to = line[pos + arrow.len()..].trim().to_string();
            // Inline label on the tail side: `from -- "text"` / `from -- text`.
            let mut label = None;
            if let Some(dp) = from.find("--") {
                let lbl = from[dp + 2..].trim().trim_matches('"').trim().to_string();
                from = from[..dp].trim().to_string();
                if !lbl.is_empty() {
                    label = Some(lbl);
                }
            }
            if from.is_empty() || to.is_empty() {
                return None;
            }
            return Some(BlockEdge {
                from,
                to,
                label,
                arrow: arrow == "-->",
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_grid() {
        let d = parse("block-beta\ncolumns 3\na b c\nd[\"wide\"]:2 e\n").unwrap();
        assert_eq!(d.columns, Some(3));
        assert_eq!(d.items.len(), 5);
        match &d.items[3] {
            BlockItem::Block(b) => {
                assert_eq!(b.id, "d");
                assert_eq!(b.label, "wide");
                assert_eq!(b.span, 2);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn group() {
        let d = parse("block-beta\nblock:g\n  x y\nend\n").unwrap();
        assert_eq!(d.items.len(), 1);
        match &d.items[0] {
            BlockItem::Group(g) => {
                assert_eq!(g.id, "g");
                assert_eq!(g.items.len(), 2);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn classdef_class_and_style_not_ghost_blocks() {
        let d = parse(
            "block-beta\n  a b\n  classDef blue fill:#66f,stroke:#333\n  class a blue\n  style b fill:#0f0\n",
        )
        .unwrap();
        // Only the two real blocks survive — no ghost blocks for the keywords.
        assert_eq!(d.items.len(), 2);
        assert!(d.class_defs.contains_key("blue"));
        match &d.items[0] {
            BlockItem::Block(b) => assert_eq!(b.classes, vec!["blue".to_string()]),
            _ => panic!(),
        }
        match &d.items[1] {
            BlockItem::Block(b) => assert_eq!(b.style, vec![("fill".into(), "#0f0".into())]),
            _ => panic!(),
        }
    }

    #[test]
    fn inline_class_shorthand() {
        let d = parse("block-beta\n  a[\"A\"]:::warn\n").unwrap();
        match &d.items[0] {
            BlockItem::Block(b) => {
                assert_eq!(b.id, "a");
                assert_eq!(b.label, "A");
                assert_eq!(b.classes, vec!["warn".to_string()]);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn edge_label_keeps_edge() {
        let d = parse("block-beta\n  a b\n  a -- \"hello\" --> b\n").unwrap();
        let edges: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                BlockItem::Edge(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "a");
        assert_eq!(edges[0].to, "b");
        assert_eq!(edges[0].label.as_deref(), Some("hello"));
        assert!(edges[0].arrow);
    }

    #[test]
    fn block_arrow_parses() {
        let d = parse("block-beta\n  blockArrowId<[\"label\"]>(right)\n").unwrap();
        match &d.items[0] {
            BlockItem::Block(b) => {
                assert_eq!(b.id, "blockArrowId");
                assert_eq!(b.label, "label");
                assert!(matches!(
                    b.shape,
                    BlockShape::Arrow(a) if a.right && !a.left && !a.up && !a.down
                ));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn composite_block_span_kept() {
        let d = parse("block-beta\n  block:wide:2\n    x\n  end\n").unwrap();
        match &d.items[0] {
            BlockItem::Group(g) => {
                assert_eq!(g.id, "wide");
                assert_eq!(g.span, 2);
            }
            _ => panic!(),
        }
    }
}
