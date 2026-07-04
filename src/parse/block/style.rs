//! block-beta styling and edge/arrow parsing: the deferred `classDef`/`class`/
//! `style` state (`Ctx`), plus `parse_edge`/`parse_shape`/`parse_block_arrow`.

use std::collections::HashMap;

use crate::parse::ast::{
    BlockArrow, BlockEdge, BlockItem, BlockLinkStyle, BlockShape, EdgeHead, Style,
};
use crate::parse::style::parse_style_props;

/// Style state gathered while scanning: `classDef` definitions plus the
/// deferred `class`/`style` assignments that target block ids (which may be
/// declared before *or* after the assignment line).
#[derive(Default)]
pub(super) struct Ctx {
    pub(super) class_defs: HashMap<String, Style>,
    class_assign: Vec<(Vec<String>, String)>,
    style_assign: Vec<(Vec<String>, Style)>,
}

/// Consume a `classDef`/`class`/`style` statement into `ctx`; returns whether
/// the line was one of those (so the caller skips node tokenizing).
pub(super) fn handle_style_line(line: &str, ctx: &mut Ctx) -> bool {
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
        if let Some((ids, props)) = rest.trim().split_once(char::is_whitespace) {
            let ids: Vec<String> = ids
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if !ids.is_empty() {
                ctx.style_assign.push((ids, parse_style_props(props)));
            }
        }
        return true;
    }
    false
}

/// Apply deferred `class`/`style` assignments onto matching blocks (recursing
/// into groups), after the whole diagram has been scanned.
pub(super) fn apply_assignments(items: &mut [BlockItem], ctx: &Ctx) {
    for it in items {
        match it {
            BlockItem::Block(b) => {
                for (ids, name) in &ctx.class_assign {
                    if ids.iter().any(|i| i == &b.id) && !b.classes.contains(name) {
                        b.classes.push(name.clone());
                    }
                }
                for (ids, style) in &ctx.style_assign {
                    if ids.iter().any(|i| i == &b.id) {
                        b.style.extend(style.iter().cloned());
                    }
                }
            }
            BlockItem::Group(g) => apply_assignments(&mut g.items, ctx),
            _ => {}
        }
    }
}

/// Parse a block arrow `id<["label"]>(dir[, dir…])`. Returns `(id, arrow, label)`.
pub(super) fn parse_block_arrow(s: &str) -> Option<(String, BlockArrow, String)> {
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

pub(super) fn parse_shape(s: &str) -> (String, BlockShape, String) {
    let s = s.trim();
    // `>` opens the asymmetric `>text]` shape; the classic delimiters open the
    // rest. The id is everything before the first opener.
    let shape_start = s.find(['[', '(', '{', '>']);
    let (id, shape_part) = match shape_start {
        Some(p) => (&s[..p], &s[p..]),
        None => (s, ""),
    };
    // (open_len, close_delim, shape) — longest openers first so `[[`/`(((`
    // win over the single-char forms.
    let (shape, label_raw) = if shape_part.is_empty() {
        (BlockShape::Rect, id.to_string())
    } else if let Some(inner) = strip_pair(shape_part, "[[", "]]") {
        (BlockShape::Subroutine, inner)
    } else if let Some(inner) = strip_pair(shape_part, "(((", ")))") {
        (BlockShape::DoubleCircle, inner)
    } else if let Some(inner) = strip_pair(shape_part, "[(", ")]") {
        (BlockShape::Cylinder, inner)
    } else if let Some(inner) = strip_pair(shape_part, "((", "))") {
        (BlockShape::Circle, inner)
    } else if let Some(inner) = strip_pair(shape_part, "([", "])") {
        (BlockShape::Stadium, inner)
    } else if let Some(inner) = strip_pair(shape_part, "{{", "}}") {
        (BlockShape::Hexagon, inner)
    } else if let Some(inner) = strip_pair(shape_part, "[/", "/]") {
        (BlockShape::LeanRight, inner)
    } else if let Some(inner) = strip_pair(shape_part, "[/", "\\]") {
        (BlockShape::Trapezoid, inner)
    } else if let Some(inner) = strip_pair(shape_part, "[\\", "\\]") {
        (BlockShape::LeanLeft, inner)
    } else if let Some(inner) = strip_pair(shape_part, "[\\", "/]") {
        (BlockShape::TrapezoidAlt, inner)
    } else if let Some(inner) = strip_pair(shape_part, ">", "]") {
        (BlockShape::Odd, inner)
    } else if let Some(inner) = strip_pair(shape_part, "[", "]") {
        (BlockShape::Rect, inner)
    } else if let Some(inner) = strip_pair(shape_part, "(", ")") {
        (BlockShape::Round, inner)
    } else if let Some(inner) = strip_pair(shape_part, "{", "}") {
        (BlockShape::Rhombus, inner)
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

/// If `s` starts with `open` and ends with `close` (and is long enough to hold
/// both without overlap), return the inner text.
fn strip_pair(s: &str, open: &str, close: &str) -> Option<String> {
    if s.starts_with(open) && s.ends_with(close) && s.len() >= open.len() + close.len() {
        Some(s[open.len()..s.len() - close.len()].to_string())
    } else {
        None
    }
}

pub(super) fn parse_edge(line: &str) -> Option<BlockEdge> {
    // Scan left-to-right for the first link operator. The connectors are all
    // ASCII (`- = . ~ x o < >`), so byte indexing is safe.
    let b = line.as_bytes();
    for i in 0..b.len() {
        // A leading `x`/`o`/`<` only counts as a tail marker at a token
        // boundary, so an id ending in `o` (e.g. `foo`) can't be mistaken for
        // one when the `--` core follows.
        let boundary = i == 0 || b[i - 1].is_ascii_whitespace();
        if let Some((end, tail, style, head)) = match_link(b, i, boundary) {
            let mut from = line[..i].trim().to_string();
            let to = line[end..].trim().to_string();
            // Inline label on the tail side: `from -- "text"` / `from -. text` /
            // `from == text`.
            let label = extract_inline_label(&mut from);
            if from.is_empty() || to.is_empty() {
                continue;
            }
            return Some(BlockEdge {
                from,
                to,
                label,
                tail,
                head,
                style,
            });
        }
    }
    None
}

/// Match a link operator starting at byte `i`, returning `(end, tail, style,
/// head)` where `end` is the byte index just past the operator. `boundary` says
/// whether a leading `x`/`o`/`<` may act as a tail marker (solid links only,
/// mirroring the upstream `[xo<]?--+[-xo>]` pattern; thick/dotted carry no
/// tail).
fn match_link(
    b: &[u8],
    i: usize,
    boundary: bool,
) -> Option<(usize, EdgeHead, BlockLinkStyle, EdgeHead)> {
    if boundary {
        let tail = match b.get(i) {
            Some(b'<') => Some(EdgeHead::Arrow),
            Some(b'o') => Some(EdgeHead::Circle),
            Some(b'x') => Some(EdgeHead::Cross),
            _ => None,
        };
        if let Some(tail) = tail {
            if let Some((end, style, head)) = match_core(b, i + 1) {
                if style == BlockLinkStyle::Solid {
                    return Some((end, tail, style, head));
                }
            }
        }
    }
    let (end, style, head) = match_core(b, i)?;
    Some((end, EdgeHead::None, style, head))
}

/// Match a tail-less link core (`~~~`, `-.-`/`-.->`, `==…`, `--…`) at byte `i`.
fn match_core(b: &[u8], i: usize) -> Option<(usize, BlockLinkStyle, EdgeHead)> {
    let rest = &b[i..];
    if rest.starts_with(b"~~~") {
        let mut j = i;
        while b.get(j) == Some(&b'~') {
            j += 1;
        }
        return Some((j, BlockLinkStyle::Invisible, EdgeHead::None));
    }
    if rest.starts_with(b"-.->") {
        return Some((i + 4, BlockLinkStyle::Dotted, EdgeHead::Arrow));
    }
    if rest.starts_with(b"-.-") {
        return Some((i + 3, BlockLinkStyle::Dotted, EdgeHead::None));
    }
    if rest.starts_with(b"==") {
        let head = head_char(b.get(i + 2), b'=')?;
        return Some((i + 3, BlockLinkStyle::Thick, head));
    }
    if rest.starts_with(b"--") {
        let head = head_char(b.get(i + 2), b'-')?;
        return Some((i + 3, BlockLinkStyle::Solid, head));
    }
    None
}

/// Classify the head char after a `--`/`==` core. `filler` spells "no head" for
/// this line style (`-` for solid `---`, `=` for thick `===`). A core not
/// followed by a valid head char (e.g. the `--` of a `-- "text"` label opener)
/// is not a link.
fn head_char(c: Option<&u8>, filler: u8) -> Option<EdgeHead> {
    match c {
        Some(b'>') => Some(EdgeHead::Arrow),
        Some(b'x') => Some(EdgeHead::Cross),
        Some(b'o') => Some(EdgeHead::Circle),
        Some(&c) if c == filler => Some(EdgeHead::None),
        _ => None,
    }
}

/// Split a trailing inline-label opener (`--`/`-.`/`==`) off the tail side of a
/// link's `from` text, returning the label text (if any).
fn extract_inline_label(from: &mut String) -> Option<String> {
    for open in ["--", "-.", "=="] {
        if let Some(dp) = from.find(open) {
            let lbl = from[dp + open.len()..]
                .trim()
                .trim_matches('"')
                .trim()
                .to_string();
            *from = from[..dp].trim().to_string();
            return (!lbl.is_empty()).then_some(lbl);
        }
    }
    None
}
