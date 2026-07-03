//! block-beta styling and edge/arrow parsing: the deferred `classDef`/`class`/
//! `style` state (`Ctx`), plus `parse_edge`/`parse_shape`/`parse_block_arrow`.

use std::collections::HashMap;

use crate::parse::ast::{BlockArrow, BlockEdge, BlockItem, BlockShape, Style};
use crate::parse::style::parse_style_props;

/// Style state gathered while scanning: `classDef` definitions plus the
/// deferred `class`/`style` assignments that target block ids (which may be
/// declared before *or* after the assignment line).
#[derive(Default)]
pub(super) struct Ctx {
    pub(super) class_defs: HashMap<String, Style>,
    class_assign: Vec<(Vec<String>, String)>,
    style_assign: Vec<(String, Style)>,
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
pub(super) fn apply_assignments(items: &mut [BlockItem], ctx: &Ctx) {
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

pub(super) fn parse_edge(line: &str) -> Option<BlockEdge> {
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
