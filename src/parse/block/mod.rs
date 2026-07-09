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

mod blocks;
mod style;
#[cfg(test)]
mod tests;

use super::ast::{BlockDiagram, BlockGroup, BlockItem};
use super::{strip_comment, ParseError};

use blocks::parse_block_line;
use style::{apply_assignments, handle_style_line, parse_edge, Ctx};

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
