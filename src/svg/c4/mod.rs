//! C4 diagram renderer.
//!
//! Layout: every diagram uses upstream's row-flow placement — each level lays
//! its shapes out left-to-right, wrapping after `SHAPE_IN_ROW` shapes, and is
//! then sized from that content (so, e.g., a system and the external system it
//! feeds sit side by side, and only the overflow wraps to the next row — #327).
//! Boundaries recurse the same way and are flowed `BOUNDARY_IN_ROW` per row, so
//! sibling boxes never overlap by construction.
//!
//! Boundaries are drawn as an outline around their content: dashed `7.0,7.0` for
//! most kinds, but solid for `Deployment_Node` (matching upstream's `nodeType`
//! special-case). Stroke is `#444444`, width 1.
//!
//! Relations are `#444444` straight connectors between the placed shapes,
//! clipped to each node's rectangle, with an arrow head on the destination side
//! (and on the source side for `BiRel`). Labels sit at the segment midpoint on an
//! opaque background so they stay legible where they cross geometry; `[techn]`
//! renders italic below the label.

use std::collections::HashMap;

use crate::parse::{
    C4BoundaryKind, C4Diagram, C4Element, C4ElementKind, C4ElementStyle, C4RelStyle,
};

use super::builder::{fnum, SvgBuilder};
use super::metrics::text_width;
use super::theme::Theme;

mod rel;
mod shapes;
#[cfg(test)]
mod tests;

use rel::draw_rel;
use shapes::{draw_element, resolve_element_style};

const PAD: f64 = 32.0;
const TITLE_GAP: f64 = 44.0;

const BOX_W: f64 = 220.0;
const BOX_H: f64 = 130.0;

const BOUNDARY_HDR: f64 = 28.0;
const BOUNDARY_PAD: f64 = 20.0;
const BOUNDARY_MIN_W: f64 = 200.0;

// Row-flow knobs — upstream defaults (settable via UpdateLayoutConfig, see #14).
const SHAPE_IN_ROW: usize = 4;
const BOUNDARY_IN_ROW: usize = 2;

const H_GAP: f64 = 40.0;
const V_GAP: f64 = 40.0;

/// Boundary outlines and relation lines both use upstream's `#444444`.
pub(super) const C4_LINE: &str = "#444444";

pub(crate) fn render(d: &C4Diagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };

    let origin_x = PAD;
    let origin_y = PAD + title_h;

    let mut pos: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    let mut boundaries: Vec<BoundaryBox> = Vec::new();
    let mut leaves: Vec<(C4Element, f64, f64, f64, f64)> = Vec::new();

    // Every diagram uses the upstream row-flow placement (#327). Row-flow knobs
    // are overridable via `UpdateLayoutConfig` (see #14).
    let shape_in_row = d.layout.shape_in_row.unwrap_or(SHAPE_IN_ROW).max(1);
    let boundary_in_row = d.layout.boundary_in_row.unwrap_or(BOUNDARY_IN_ROW).max(1);
    let (nodes, _cw, _ch) =
        flow_layout(&d.elements, shape_in_row, boundary_in_row, theme.font_size);
    place_absolute(
        &nodes,
        origin_x,
        origin_y,
        &mut pos,
        &mut boundaries,
        &mut leaves,
    );

    let mut max_x = origin_x;
    let mut max_y = origin_y;
    for &(x, y, w, h) in pos.values() {
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }

    let width = (max_x + PAD).max(600.0);
    let height = (max_y + PAD).max(220.0);
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    svg.def_arrow_marker("c4-arrow", C4_LINE, 9, 9);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 22.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\""),
            t,
        );
    }

    let boundary_styles: HashMap<&str, &C4ElementStyle> = d
        .boundary_styles
        .iter()
        .map(|s| (s.alias.as_str(), s))
        .collect();
    for b in &boundaries {
        let ov = boundary_styles.get(b.alias.as_str()).copied();
        draw_boundary_rect(b, ov, &mut svg, theme);
    }

    let elem_styles: HashMap<&str, &C4ElementStyle> = d
        .element_styles
        .iter()
        .map(|s| (s.alias.as_str(), s))
        .collect();
    let rel_styles: HashMap<(&str, &str), &C4RelStyle> = d
        .rel_styles
        .iter()
        .map(|s| ((s.from.as_str(), s.to.as_str()), s))
        .collect();

    for (el, x, y, w, h) in &leaves {
        let ov = elem_styles.get(el.alias.as_str()).copied();
        let style = resolve_element_style(el, ov);
        draw_element(el, &style, *x, *y, *w, *h, &mut svg, theme);
    }

    for r in &d.relations {
        let ov = rel_styles.get(&(r.from.as_str(), r.to.as_str())).copied();
        draw_rel(r, ov, &pos, &mut svg, theme);
    }

    svg.finish()
}

struct BoundaryBox {
    alias: String,
    label: String,
    kind: C4BoundaryKind,
    /// Explicit type argument (overrides the fixed per-kind `[label]` tag).
    type_text: Option<String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A placed element in the row-flow layout. Coordinates are relative to the
/// top-left origin of the level the node belongs to; a boundary's `children`
/// are relative to the boundary's own top-left.
struct LayoutNode {
    el: C4Element,
    is_boundary: bool,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    children: Vec<LayoutNode>,
}

/// Lay out `items` with the upstream row-flow: size each item (recursively for
/// boundaries), then place items left-to-right, wrapping after `SHAPE_IN_ROW`
/// shapes / `BOUNDARY_IN_ROW` boundaries. Returns the placed nodes plus the
/// total content size.
fn flow_layout(
    items: &[C4Element],
    shape_in_row: usize,
    boundary_in_row: usize,
    font_size: f64,
) -> (Vec<LayoutNode>, f64, f64) {
    let mut nodes: Vec<LayoutNode> = items
        .iter()
        .map(|item| {
            if item.boundary_kind.is_some() {
                let (mut kids, cw, ch) =
                    flow_layout(&item.members, shape_in_row, boundary_in_row, font_size);
                let dx = BOUNDARY_PAD;
                let dy = BOUNDARY_PAD + BOUNDARY_HDR;
                for k in &mut kids {
                    k.x += dx;
                    k.y += dy;
                }
                let w = (cw + 2.0 * BOUNDARY_PAD)
                    .max(BOUNDARY_MIN_W)
                    .max(header_min_w(item, font_size));
                let h = ch + 2.0 * BOUNDARY_PAD + BOUNDARY_HDR;
                LayoutNode {
                    el: item.clone(),
                    is_boundary: true,
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                    children: kids,
                }
            } else {
                let (w, h) = shape_size(item.kind);
                LayoutNode {
                    el: item.clone(),
                    is_boundary: false,
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                    children: Vec::new(),
                }
            }
        })
        .collect();

    let mut x = 0.0;
    let mut y = 0.0;
    let mut row_h: f64 = 0.0;
    let mut col = 0usize;
    let mut total_w: f64 = 0.0;
    for node in &mut nodes {
        let per_row = if node.is_boundary {
            boundary_in_row
        } else {
            shape_in_row
        };
        if col >= per_row {
            x = 0.0;
            y += row_h + V_GAP;
            row_h = 0.0;
            col = 0;
        }
        node.x = x;
        node.y = y;
        x += node.w + H_GAP;
        row_h = row_h.max(node.h);
        total_w = total_w.max(x - H_GAP);
        col += 1;
    }
    let total_h = y + row_h;
    (nodes, total_w, total_h)
}

/// Walk the placed tree, converting relative coords to absolute canvas coords
/// and collecting boundary frames, leaf draw entries, and the alias → rect map
/// used to route relations.
fn place_absolute(
    nodes: &[LayoutNode],
    ox: f64,
    oy: f64,
    pos: &mut HashMap<String, (f64, f64, f64, f64)>,
    boundaries: &mut Vec<BoundaryBox>,
    leaves: &mut Vec<(C4Element, f64, f64, f64, f64)>,
) {
    for n in nodes {
        let ax = ox + n.x;
        let ay = oy + n.y;
        pos.insert(n.el.alias.clone(), (ax, ay, n.w, n.h));
        if n.is_boundary {
            boundaries.push(BoundaryBox {
                alias: n.el.alias.clone(),
                label: n.el.label.clone(),
                kind: n.el.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
                type_text: n.el.boundary_type.clone(),
                x: ax,
                y: ay,
                w: n.w,
                h: n.h,
            });
            place_absolute(&n.children, ax, ay, pos, boundaries, leaves);
        } else {
            leaves.push((n.el.clone(), ax, ay, n.w, n.h));
        }
    }
}

fn shape_size(_kind: C4ElementKind) -> (f64, f64) {
    (BOX_W, BOX_H)
}

/// Minimum boundary width so the header label and the `[kind]` tag (stacked
/// below it) don't get clipped.
fn header_min_w(b: &C4Element, font_size: f64) -> f64 {
    let kind = boundary_tag_text(
        b.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
        &b.boundary_type,
    );
    let label_w = text_width(&b.label, 8.0, font_size);
    let kind_w = text_width(&kind, 6.0, font_size);
    label_w.max(kind_w) + 28.0
}

/// The `[…]` header tag: an explicit type argument overrides the fixed per-kind
/// label (e.g. `Deployment_Node(n, "Web Server", "Ubuntu 16.04 LTS")` shows the
/// OS text).
fn boundary_tag_text(kind: C4BoundaryKind, type_text: &Option<String>) -> String {
    match type_text {
        Some(t) if !t.is_empty() => t.clone(),
        _ => boundary_kind_label(kind).to_string(),
    }
}

fn boundary_kind_label(kind: C4BoundaryKind) -> &'static str {
    match kind {
        C4BoundaryKind::Enterprise => "Enterprise Boundary",
        C4BoundaryKind::System => "System Boundary",
        C4BoundaryKind::Container => "Container Boundary",
        C4BoundaryKind::Deployment => "Deployment Node",
        C4BoundaryKind::Generic => "Boundary",
    }
}

fn draw_boundary_rect(
    b: &BoundaryBox,
    style: Option<&C4ElementStyle>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    // `UpdateBoundaryStyle` overrides the outline/fill/label colors.
    let fill = style
        .and_then(|s| s.bg_color.clone())
        .unwrap_or_else(|| "none".to_string());
    let stroke = style
        .and_then(|s| s.border_color.clone())
        .unwrap_or_else(|| C4_LINE.to_string());
    let fg = style
        .and_then(|s| s.font_color.clone())
        .unwrap_or_else(|| theme.fg.to_string());
    let fg_muted = &theme.fg_muted;
    // Upstream draws a `Deployment_Node` (any boundary with a `nodeType`) with a
    // solid border and every other boundary kind dashed `7.0,7.0`.
    let dash = if matches!(b.kind, C4BoundaryKind::Deployment) {
        String::new()
    } else {
        " stroke-dasharray=\"7 7\"".to_string()
    };
    svg.rect(
        b.x,
        b.y,
        b.w,
        b.h,
        &format!(
            "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1\" rx=\"2.5\" ry=\"2.5\"{dash}"
        ),
    );
    let kind = boundary_tag_text(b.kind, &b.type_text);
    let label_size = theme.font_size + 2.0;
    svg.text(
        b.x + 14.0,
        b.y + 18.0,
        &format!(
            "fill=\"{fg}\" font-size=\"{}\" font-weight=\"bold\"",
            fnum(label_size)
        ),
        &b.label,
    );
    svg.text(
        b.x + 14.0,
        b.y + 18.0 + label_size,
        &format!("fill=\"{fg_muted}\" font-size=\"10\" font-style=\"italic\""),
        &format!("[{kind}]"),
    );
}
