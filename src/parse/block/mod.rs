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

mod style;

use super::ast::{Block, BlockDiagram, BlockGroup, BlockItem, BlockShape, Style};
use super::{strip_comment, ParseError};

use style::{
    apply_assignments, handle_style_line, parse_block_arrow, parse_edge, parse_shape, Ctx,
};

pub(crate) fn parse(input: &str) -> Result<BlockDiagram, ParseError> {
    let mut d = BlockDiagram::default();
    let mut ctx = Ctx::default();
    let mut header_seen = false;
    let mut auto_cols = false;
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
                return Err(ParseError::header(line_no, "expected 'block-beta' header"));
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("columns") {
            let arg = rest.trim();
            if arg == "auto" {
                // Upstream `-1`: pack every top-level cell into one row.
                auto_cols = true;
            } else {
                let v: usize = arg.parse().map_err(|_| {
                    ParseError::number(line_no, format!("invalid columns: '{arg}'"))
                })?;
                d.columns = Some(v);
            }
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
    if auto_cols {
        d.columns = Some(auto_column_count(&d.items).max(1));
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
    let mut auto_cols = false;
    let mut i = 0;
    while i < body.len() {
        let line = body[i].1.trim().to_string();
        i += 1;
        if let Some(rest) = line.strip_prefix("columns") {
            if rest.trim() == "auto" {
                auto_cols = true;
            } else {
                columns = rest.trim().parse().ok();
            }
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
    if auto_cols {
        columns = Some(auto_column_count(&items).max(1));
    }
    Ok(BlockGroup {
        id,
        label: None,
        columns,
        items,
        span,
    })
}

/// `columns auto`: total cell width of a container's direct items — blocks and
/// groups by span, spaces by count, edges contribute nothing.
fn auto_column_count(items: &[BlockItem]) -> usize {
    items
        .iter()
        .map(|it| match it {
            BlockItem::Block(b) => b.span.max(1),
            BlockItem::Group(g) => g.span.max(1),
            BlockItem::Space(n) => *n,
            BlockItem::Edge(_) => 0,
        })
        .sum()
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
                // Floor at 0 so the unmatched `]` of an asymmetric `>text]`
                // shape doesn't drive depth negative and glue tokens together.
                ']' | ')' | '}' => depth = (depth - 1).max(0),
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
    // `space` is a keyword only on its own or as `space:N` — not a prefix, so
    // ids like `spaceship` are left intact.
    if tok == "space" {
        return Some(BlockItem::Space(1));
    }
    if let Some(rest) = tok.strip_prefix("space:") {
        return Some(BlockItem::Space(rest.trim().parse::<usize>().unwrap_or(1)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ast::BlockLinkStyle;

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

    fn blocks(d: &BlockDiagram) -> Vec<&Block> {
        d.items
            .iter()
            .filter_map(|i| match i {
                BlockItem::Block(b) => Some(b),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn subroutine_and_double_circle_shapes() {
        let d = parse("block-beta\n  a[[\"subroutine\"]] b(((\"double\")))\n").unwrap();
        let bs = blocks(&d);
        assert_eq!(bs[0].label, "subroutine");
        assert_eq!(bs[0].shape, BlockShape::Subroutine);
        assert_eq!(bs[1].label, "double");
        assert_eq!(bs[1].shape, BlockShape::DoubleCircle);
    }

    #[test]
    fn asymmetric_and_lean_shapes_dont_mangle_line() {
        let d = parse(
            "block-beta\n  a>\"asym\"] b[/\"lr\"/] c[\\\"ll\"\\] d[/\"tz\"\\] e[\\\"tza\"/]\n",
        )
        .unwrap();
        let bs = blocks(&d);
        assert_eq!(bs.len(), 5);
        assert_eq!(bs[0].shape, BlockShape::Odd);
        assert_eq!(bs[0].label, "asym");
        assert_eq!(bs[1].shape, BlockShape::LeanRight);
        assert_eq!(bs[2].shape, BlockShape::LeanLeft);
        assert_eq!(bs[3].shape, BlockShape::Trapezoid);
        assert_eq!(bs[4].shape, BlockShape::TrapezoidAlt);
    }

    #[test]
    fn dotted_thick_invisible_links() {
        let d = parse("block-beta\n  a b c d\n  a -.-> b\n  b ==> c\n  c ~~~ d\n").unwrap();
        let edges: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                BlockItem::Edge(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0].style, BlockLinkStyle::Dotted);
        assert!(edges[0].arrow);
        assert_eq!(edges[1].style, BlockLinkStyle::Thick);
        assert_eq!(edges[2].style, BlockLinkStyle::Invisible);
    }

    #[test]
    fn columns_auto_packs_one_row() {
        let d = parse("block-beta\n  columns auto\n  a b c d\n").unwrap();
        assert_eq!(d.columns, Some(4));
    }

    #[test]
    fn space_prefixed_id_is_not_a_space() {
        let d = parse("block-beta\n  spaceship[\"Ship\"] space\n").unwrap();
        let bs = blocks(&d);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].id, "spaceship");
        assert_eq!(bs[0].label, "Ship");
        assert!(matches!(d.items[1], BlockItem::Space(1)));
    }

    #[test]
    fn style_multi_id_list() {
        let d = parse("block-beta\n  a b\n  style a,b fill:#f00\n").unwrap();
        let bs = blocks(&d);
        assert_eq!(bs[0].style, vec![("fill".into(), "#f00".into())]);
        assert_eq!(bs[1].style, vec![("fill".into(), "#f00".into())]);
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
