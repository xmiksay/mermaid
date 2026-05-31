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

use super::ast::{Block, BlockDiagram, BlockEdge, BlockGroup, BlockItem, BlockShape};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<BlockDiagram, ParseError> {
    let mut d = BlockDiagram::default();
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

        if let Some(rest) = line.strip_prefix("block:") {
            let id = rest
                .trim()
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let inner = collect_until_end(&lines, &mut i);
            let group = parse_group(id, &inner)?;
            d.items.push(BlockItem::Group(group));
            continue;
        }
        if line == "block" {
            let inner = collect_until_end(&lines, &mut i);
            let group = parse_group(String::new(), &inner)?;
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
    Ok(d)
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

fn parse_group(id: String, body: &[(usize, String)]) -> Result<BlockGroup, ParseError> {
    let mut items: Vec<BlockItem> = Vec::new();
    let mut columns: Option<usize> = None;
    let mut i = 0;
    while i < body.len() {
        let (line_no, line) = (body[i].0, body[i].1.trim().to_string());
        i += 1;
        if let Some(rest) = line.strip_prefix("columns") {
            columns = rest.trim().parse().ok();
            let _ = line_no;
            continue;
        }
        if let Some(rest) = line.strip_prefix("block:") {
            let nid = rest
                .trim()
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let inner = collect_until_end(body, &mut i);
            items.push(BlockItem::Group(parse_group(nid, &inner)?));
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
    }))
}

fn parse_shape(s: &str) -> (String, BlockShape, String) {
    let s = s.trim();
    let shape_start = s.find(|c| matches!(c, '[' | '(' | '{'));
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
            let from = line[..pos].trim().to_string();
            let to = line[pos + arrow.len()..].trim().to_string();
            return Some(BlockEdge {
                from,
                to,
                label: None,
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
}
