//! block-beta node tokenizing: split a line into block items and parse a
//! single `id[shape]:span` token.

use crate::parse::ast::{Block, BlockItem, BlockShape, Style};

use super::style::{parse_block_arrow, parse_shape};

pub(super) fn parse_block_line(line: &str) -> Vec<BlockItem> {
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
