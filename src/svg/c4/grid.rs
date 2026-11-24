//! Grid mode: upstream's row-flow placement (#327/#370). Each level flows its
//! shapes left-to-right, wrapping after `shape_in_row` shapes or once the row
//! outgrows `SHAPE_ROW_WIDTH_LIMIT`; boundaries recurse the same way and wrap
//! after `boundary_in_row`, so sibling boxes never overlap by construction.

use std::collections::HashMap;

use crate::parse::{C4BoundaryKind, C4Diagram, C4Element};

use super::rel_loop::enclosure_room;
use super::rel_loop::loop_room;
use super::text::shape_size;
use super::{header_min_w, BoundaryBox, Placement, BOUNDARY_HDR, BOUNDARY_MIN_W, BOUNDARY_PAD};

// Row-flow knobs — upstream defaults (settable via UpdateLayoutConfig, see #14).
const SHAPE_IN_ROW: usize = 4;
const BOUNDARY_IN_ROW: usize = 2;

const H_GAP: f64 = 40.0;
const V_GAP: f64 = 40.0;

// Upstream wraps a shape row not just after `c4ShapeInRow` shapes but also
// once the row's content would outgrow the render surface — its `Bounds.insert`
// wraps as soon as a shape's `stopx` would reach `screen.availWidth` (the
// browser's screen width). In a headless render that's the default ~800px
// viewport, which — with upstream's own default `width: 216` and
// `c4ShapeMargin: 50` — already wraps after two same-sized shapes, well
// before the `c4ShapeInRow` default of 4 ever applies. That's why upstream
// tiers `samples/c4.mmd` into short rows (persons, then systems two at a
// time) instead of one wide row (#370). `SHAPE_ROW_WIDTH_LIMIT` mirrors that
// with our own `BOX_W`/`H_GAP`: any value in (2·BOX_W + H_GAP, 3·BOX_W +
// 2·H_GAP] reproduces the same two-per-row grouping.
const SHAPE_ROW_WIDTH_LIMIT: f64 = 700.0;

/// A placed element. Coordinates are relative to the top-left origin of the
/// level the node belongs to; a boundary's `children` are relative to the
/// boundary's own top-left.
struct LayoutNode {
    el: C4Element,
    is_boundary: bool,
    /// Extra room kept free on both sides (a self-relation's loop, which
    /// takes whichever side is free).
    reserve: f64,
    /// A boundary's header text width.
    header_w: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    children: Vec<LayoutNode>,
}

struct Flow<'a> {
    loops: HashMap<&'a str, f64>,
    /// Extra bottom padding a boundary needs (`enclosure_room`).
    enclosures: HashMap<&'a str, f64>,
    shape_in_row: usize,
    boundary_in_row: usize,
    font_size: f64,
    v_gap: f64,
    h_gap: f64,
}

/// Lay `d` out on the row-flow grid with its top-left content corner at
/// `origin`. `min_gaps` = `(column, row)` floors for the gaps (see
/// `min_label_gaps`).
pub(super) fn grid_placement(
    d: &C4Diagram,
    origin: (f64, f64),
    font_size: f64,
    min_gaps: (f64, f64),
) -> Placement {
    let flow = Flow {
        loops: loop_room(&d.relations, false),
        enclosures: enclosure_room(&d.elements, &d.relations, false),
        shape_in_row: d.layout.shape_in_row.unwrap_or(SHAPE_IN_ROW).max(1),
        boundary_in_row: d.layout.boundary_in_row.unwrap_or(BOUNDARY_IN_ROW).max(1),
        font_size,
        v_gap: V_GAP.max(min_gaps.1),
        h_gap: H_GAP.max(min_gaps.0),
    };
    let (nodes, _, _) = flow_layout(&d.elements, &flow);
    let mut out = Placement {
        routes: vec![None; d.relations.len()],
        ..Placement::default()
    };
    place_absolute(&nodes, origin.0, origin.1, &mut out);
    out
}

/// Size each item (recursively for boundaries), then place items
/// left-to-right with wrapping. Shapes in one row are stretched to the row's
/// tallest shape so the grid keeps upstream's even rows while every box still
/// fits its own text. Returns the placed nodes plus the total content size.
fn flow_layout(items: &[C4Element], flow: &Flow<'_>) -> (Vec<LayoutNode>, f64, f64) {
    let mut nodes: Vec<LayoutNode> = items.iter().map(|item| size_node(item, flow)).collect();

    let mut x = 0.0;
    let mut y = 0.0;
    let mut row_h: f64 = 0.0;
    let mut row_start = 0usize;
    let mut col = 0usize;
    let mut total_w: f64 = 0.0;
    for i in 0..nodes.len() {
        let node = &nodes[i];
        let per_row = if node.is_boundary {
            flow.boundary_in_row
        } else {
            flow.shape_in_row
        };
        // Shapes additionally wrap once the row would outgrow
        // `SHAPE_ROW_WIDTH_LIMIT`, matching upstream's screen-width-driven
        // wrap (#370); boundaries keep the plain per-row count.
        let outgrows_width =
            !node.is_boundary && col > 0 && x + node.w + 2.0 * node.reserve > SHAPE_ROW_WIDTH_LIMIT;
        if col >= per_row || outgrows_width {
            stretch_row(&mut nodes[row_start..i]);
            row_start = i;
            x = 0.0;
            y += row_h + flow.v_gap;
            row_h = 0.0;
            col = 0;
        }
        let node = &mut nodes[i];
        node.x = x;
        node.y = y;
        x += node.w + 2.0 * node.reserve + flow.h_gap;
        row_h = row_h.max(node.h);
        total_w = total_w.max(x - flow.h_gap);
        col += 1;
    }
    stretch_row(&mut nodes[row_start..]);
    (nodes, total_w, y + row_h)
}

fn stretch_row(row: &mut [LayoutNode]) {
    let tallest = row
        .iter()
        .filter(|n| !n.is_boundary)
        .map(|n| n.h)
        .fold(0.0_f64, f64::max);
    for n in row.iter_mut().filter(|n| !n.is_boundary) {
        n.h = tallest;
    }
}

fn size_node(item: &C4Element, flow: &Flow<'_>) -> LayoutNode {
    if item.boundary_kind.is_none() {
        let (w, h) = shape_size(item);
        return LayoutNode {
            el: item.clone(),
            is_boundary: false,
            reserve: flow.loops.get(item.alias.as_str()).copied().unwrap_or(0.0),
            header_w: 0.0,
            x: 0.0,
            y: 0.0,
            w,
            h,
            children: Vec::new(),
        };
    }
    let (mut kids, cw, ch) = flow_layout(&item.members, flow);
    for k in &mut kids {
        k.x += BOUNDARY_PAD;
        k.y += BOUNDARY_PAD + BOUNDARY_HDR;
    }
    let w = (cw + 2.0 * BOUNDARY_PAD)
        .max(BOUNDARY_MIN_W)
        .max(header_min_w(item, flow.font_size));
    LayoutNode {
        el: item.clone(),
        is_boundary: true,
        reserve: 0.0,
        header_w: header_min_w(item, flow.font_size) - 28.0,
        x: 0.0,
        y: 0.0,
        w,
        h: ch
            + 2.0 * BOUNDARY_PAD
            + BOUNDARY_HDR
            + flow
                .enclosures
                .get(item.alias.as_str())
                .copied()
                .unwrap_or(0.0),
        children: kids,
    }
}

/// Walk the placed tree, converting relative coords to absolute canvas coords.
fn place_absolute(nodes: &[LayoutNode], ox: f64, oy: f64, out: &mut Placement) {
    for n in nodes {
        let rect = (ox + n.x + n.reserve, oy + n.y, n.w, n.h);
        if n.is_boundary {
            out.boundaries.push(BoundaryBox {
                alias: n.el.alias.clone(),
                label: n.el.label.clone(),
                kind: n.el.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
                type_text: n.el.boundary_type.clone(),
                rect,
                header_w: n.header_w,
            });
            place_absolute(&n.children, rect.0, rect.1, out);
        } else {
            out.leaves.push((n.el.clone(), rect));
        }
    }
}
