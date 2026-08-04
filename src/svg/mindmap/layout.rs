//! Radial tree layout: place each node at the centre of its angular sector and
//! frame the whole subtree into positive space.

use crate::parse::{MindmapNode, MindmapShape};

use crate::svg::builder::{split_label_lines, LABEL_LINE_H};
use crate::svg::markup::strip_tags;
use crate::svg::metrics::{text_width, BASE_FONT_SIZE};

use super::{
    Laid, CHILD_SPREAD, ICON_GAP, ICON_SIZE, NODE_H, NODE_MARGIN, NODE_PAD_X, RING_GAP, TEXT_PX,
};

/// Label font size (px) at `depth`, scaling `base` down for deeper rings so the
/// root and first ring read largest — upstream sizes node type by depth.
fn depth_font(base: f64, depth: usize) -> f64 {
    base * match depth {
        0 => 1.2,
        1 => 1.05,
        2 => 0.95,
        _ => 0.88,
    }
}

/// Leaves in a subtree (a leaf counts as one) — the angular weight of a node.
fn leaves(n: &MindmapNode) -> usize {
    if n.children.is_empty() {
        1
    } else {
        n.children.iter().map(leaves).sum()
    }
}

/// Widest display line's measured width, honoring the same `\n`/`<br>`
/// splitting the renderer uses so a wrapped multi-line label doesn't get
/// measured as one long run (the label sizing bug behind #two-line overlap).
fn widest_line_width(text: &str, font_size: f64) -> f64 {
    split_label_lines(text)
        .iter()
        .map(|l| text_width(&strip_tags(l), TEXT_PX, font_size))
        .fold(0.0_f64, f64::max)
}

fn node_size(n: &MindmapNode, font_size: f64) -> (f64, f64) {
    let icon_w = if n.icon.is_some() {
        ICON_SIZE + ICON_GAP
    } else {
        0.0
    };
    let scale = font_size / BASE_FONT_SIZE;
    let lines = split_label_lines(&n.text).len().max(1);
    let tw = widest_line_width(&n.text, font_size);
    let w = (tw + NODE_PAD_X * 2.0 + icon_w).max(48.0 * scale);
    let h = (NODE_H + (lines as f64 - 1.0) * LABEL_LINE_H) * scale;
    (w, h)
}

/// Conservative "reach" from a node's centre to its farthest edge, safe for
/// any angle: the root is a true circle (its radius is exact in every
/// direction); every other node is an axis-aligned box, whose half-diagonal
/// bounds its edge distance regardless of which way it's approached from.
fn node_reach(w: f64, h: f64) -> f64 {
    (w * w + h * h).sqrt() / 2.0
}

/// Build the laid-out subtree for `n`, placing it at the centre of the angular
/// sector `[a0, a1)` at radius `max(depth * RING_GAP, parent_reach + margin +
/// own_reach)` and recursing on its children. The `RING_GAP` step is only a
/// floor: a node whose own measured size (or its parent's) needs more room
/// than that pushes further out, so a wide/tall label never lands on top of
/// its parent (`parent_reach` is the parent's own centre-to-edge distance,
/// `0.0` for the initial call at the root). Children of a non-root node are
/// packed into a [`CHILD_SPREAD`]-narrowed cone centred on the parent's angle
/// so each subtree stays compact around its branch node instead of sprawling
/// across the full inherited sector.
pub(super) fn build(
    n: &MindmapNode,
    depth: usize,
    section: i32,
    a0: f64,
    a1: f64,
    base_font: f64,
    parent_reach: f64,
) -> Laid {
    let angle = (a0 + a1) / 2.0;
    let font = depth_font(base_font, depth);
    let (w, h) = node_size(n, font);
    let root_r = if depth == 0 {
        (widest_line_width(&n.text, font) / 2.0 + NODE_PAD_X + 6.0).max(28.0)
    } else {
        0.0
    };
    let own_reach = if depth == 0 { root_r } else { node_reach(w, h) };
    let r = if depth == 0 {
        0.0
    } else {
        (depth as f64 * RING_GAP).max(parent_reach + NODE_MARGIN + own_reach)
    };
    let (x, y) = (r * angle.cos(), r * angle.sin());
    let reach = r + own_reach;

    // Root children fan around the full circle; deeper children hug their
    // parent's radial line within a narrowed cone.
    let (c0, c1) = if depth == 0 {
        (a0, a1)
    } else {
        let half = (a1 - a0) * 0.5 * CHILD_SPREAD;
        (angle - half, angle + half)
    };
    let total = leaves(n).max(1) as f64;
    let mut cursor = c0;
    let mut children = Vec::with_capacity(n.children.len());
    for (i, c) in n.children.iter().enumerate() {
        let span = (c1 - c0) * (leaves(c) as f64) / total;
        let child_section = if depth == 0 { i as i32 } else { section };
        children.push(build(
            c,
            depth + 1,
            child_section,
            cursor,
            cursor + span,
            base_font,
            reach,
        ));
        cursor += span;
    }

    Laid {
        node: n.clone(),
        x,
        y,
        w,
        h,
        r: root_r,
        font,
        depth,
        section,
        children,
    }
}

pub(super) fn shift(laid: &mut Laid, dx: f64, dy: f64) {
    laid.x += dx;
    laid.y += dy;
    for c in &mut laid.children {
        shift(c, dx, dy);
    }
}

pub(super) fn bounds(laid: &Laid) -> (f64, f64, f64, f64) {
    let (hw, hh) = if laid.depth == 0 {
        (laid.r, laid.r)
    } else if laid.node.shape == MindmapShape::Circle {
        // A circle node is drawn with radius `max(w, h) / 2` (see
        // `draw::draw_shape`), which reaches further vertically than a plain
        // `h / 2` box half-height whenever the label makes it wider than
        // tall — using the box half-extents here undercounted that reach and
        // let a large circle's edge get clipped by the canvas.
        let r = (laid.w / 2.0).max(laid.h / 2.0);
        (r, r)
    } else {
        (laid.w / 2.0, laid.h / 2.0)
    };
    let mut min_x = laid.x - hw;
    let mut max_x = laid.x + hw;
    let mut min_y = laid.y - hh;
    let mut max_y = laid.y + hh;
    for c in &laid.children {
        let (a, b, cc, dd) = bounds(c);
        min_x = min_x.min(a);
        min_y = min_y.min(b);
        max_x = max_x.max(cc);
        max_y = max_y.max(dd);
    }
    (min_x, min_y, max_x, max_y)
}
