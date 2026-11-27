//! Flowchart renderer: maps the AST to a `crate::sugiyama::Graph`, runs layered
//! layout, then draws shapes, clipped polyline edges, and subgraph frames.

use std::collections::HashMap;

use crate::parse::{EdgeLine, FlowDirection, FlowchartDiagram};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::SvgBuilder;
use super::style::resolve_edge_style;
use super::theme::Theme;

mod detour;
mod directions;
mod edges;
mod nodes;
mod round;
mod route;
mod shapes;
mod slots;

use detour::Obstacle;
use directions::*;
use edges::*;
use nodes::*;
use route::{route_all, EdgeIn, EdgePath};

pub(super) const CHAR_W: f64 = 7.5;
pub(super) const LINE_H: f64 = 20.0;
pub(super) const PAD_X: f64 = 18.0;
pub(super) const PAD_Y: f64 = 12.0;
pub(super) const MIN_W: f64 = 60.0;
pub(super) const MIN_H: f64 = 40.0;
const CANVAS_PAD: f64 = 24.0;
pub(super) const SUBGRAPH_PAD: f64 = 16.0;
/// Vertical space reserved above the graph for a frontmatter title.
const TITLE_BAND: f64 = 46.0;

pub(crate) fn render(d: &FlowchartDiagram, theme: &Theme) -> String {
    if d.nodes.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0).theme(theme);
        define_markers(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    // Reserve a band at the top for a frontmatter title, if present.
    let title_h = if d.title.is_some() { TITLE_BAND } else { 0.0 };
    let node_sizes: Vec<(f64, f64)> = d
        .nodes
        .iter()
        .map(|n| node_size(n, theme.font_size))
        .collect();
    let id_to_u32: HashMap<String, NodeId> = d
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i as NodeId))
        .collect();

    let nodes: Vec<NodeId> = (0..d.nodes.len() as NodeId).collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .edges
        .iter()
        .filter_map(|e| Some((*id_to_u32.get(&e.from)?, *id_to_u32.get(&e.to)?)))
        .collect();
    let node_size_su: HashMap<NodeId, (f64, f64)> = d
        .nodes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let (w, h) = node_sizes[i];
            let s = match dir {
                FlowDirection::LeftRight | FlowDirection::RightLeft => (h, w),
                _ => (w, h),
            };
            (i as NodeId, s)
        })
        .collect();

    let g = Graph {
        nodes,
        edges,
        node_size: node_size_su,
    };
    let layout = layout_with(&g, &LayoutConfig::default()).unwrap_or_default();

    let raw_h = layout.height;
    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (tx + CANVAS_PAD, ty + CANVAS_PAD + title_h)
    };

    // Screen-space node positions and edge polylines. Working in screen space
    // (rather than transforming lazily at draw time) lets a subgraph with a
    // local `direction` transpose just its own members in place.
    let mut pos: HashMap<NodeId, (f64, f64)> = (0..d.nodes.len() as NodeId)
        .map(|u| (u, transform(layout.node_pos[&u])))
        .collect();
    let mut edge_pts: HashMap<(NodeId, NodeId), Vec<(f64, f64)>> = layout
        .edge_points
        .iter()
        .map(|(k, v)| (*k, v.iter().map(|&p| transform(p)).collect()))
        .collect();

    apply_local_directions(d, dir, &id_to_u32, &mut pos, &mut edge_pts);
    resolve_subgraph_overlaps(d, &id_to_u32, &mut pos, &mut edge_pts, &node_sizes);

    let mut boxes = compute_subgraph_boxes(d, &id_to_u32, &pos, &node_sizes);

    // Canvas sizing is a measure-then-shift pass: a subgraph's local
    // `direction` can transpose its members to sit left of/above everything
    // else, and a routed edge's label can land to the right of every node, so
    // the true extent isn't known until nodes, subgraph frames (with their own
    // `SUBGRAPH_PAD`) and edge labels have all been resolved. Gather every
    // extent first, then shift the whole drawing so its minimum lands exactly
    // at `CANVAS_PAD` before the canvas is created — this is what keeps
    // negative-coordinate content (issue: subgraph/label clipping) from ever
    // reaching the SVG.
    let mut paths = route_edges(d, &id_to_u32, &node_sizes, &pos, &boxes, &edge_pts);
    let mut label_slots: Vec<super::label_layout::LabelSlot> = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for (fedge, path) in d.edges.iter().zip(&paths) {
        if let (Some(label), Some(path)) = (&fedge.label, path) {
            let w = crate::svg::metrics::text_width(label, 7.0, theme.font_size) + 8.0;
            label_slots.push(super::label_layout::LabelSlot {
                polyline: path.label.clone(),
                size: (w, 18.0),
            });
            labels.push(label);
        }
    }
    let mut label_pos = super::label_layout::layout_edge_labels(&label_slots);

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (u, &(x, y)) in &pos {
        let (w, h) = node_sizes[*u as usize];
        min_x = min_x.min(x - w / 2.0);
        max_x = max_x.max(x + w / 2.0);
        min_y = min_y.min(y - h / 2.0);
        max_y = max_y.max(y + h / 2.0);
    }
    for &(bx0, by0, bx1, by1) in boxes.values() {
        min_x = min_x.min(bx0);
        max_x = max_x.max(bx1);
        min_y = min_y.min(by0);
        max_y = max_y.max(by1);
    }
    // Measure the drawn curves, not their control points, so the canvas
    // only moves for an edge that really overflows.
    for (x, y) in paths.iter().flatten().flat_map(|p| p.drawn_points()) {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    for (slot, &(cx, cy)) in label_slots.iter().zip(&label_pos) {
        let (w, h) = slot.size;
        min_x = min_x.min(cx - w / 2.0);
        max_x = max_x.max(cx + w / 2.0);
        min_y = min_y.min(cy - h / 2.0);
        max_y = max_y.max(cy + h / 2.0);
    }

    // Normalize so the measured minimum sits exactly at the canvas padding —
    // a no-op shift when nothing overhangs, since `pos` is already offset by
    // `CANVAS_PAD` via `transform`.
    let shift_x = CANVAS_PAD - min_x;
    let shift_y = (CANVAS_PAD + title_h) - min_y;
    for v in pos.values_mut() {
        v.0 += shift_x;
        v.1 += shift_y;
    }
    for path in paths.iter_mut().flatten() {
        path.shift(shift_x, shift_y);
    }
    for p in &mut label_pos {
        p.0 += shift_x;
        p.1 += shift_y;
    }
    for b in boxes.values_mut() {
        b.0 += shift_x;
        b.1 += shift_y;
        b.2 += shift_x;
        b.3 += shift_y;
    }

    let mut width = (max_x + shift_x) + CANVAS_PAD;
    let height = (max_y + shift_y) + CANVAS_PAD;

    // A long title can be wider than the graph itself; grow the canvas to fit.
    if let Some(t) = &d.title {
        let title_w =
            crate::svg::metrics::text_width(t, CHAR_W + 2.0, theme.font_size) + CANVAS_PAD * 2.0;
        width = width.max(title_w);
    }

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    define_markers(&mut svg, theme);

    if let Some(t) = &d.title {
        let fg = &theme.fg;
        svg.text(
            width / 2.0,
            CANVAS_PAD + 6.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    // Subgraph frames (drawn first so they sit under nodes/edges).
    draw_subgraphs(&mut svg, d, &boxes, theme);

    // Edges first, then their labels on top.
    for (fedge, path) in d.edges.iter().zip(&paths) {
        if let Some(path) = path {
            draw_edge(&mut svg, path, fedge, theme);
        }
    }
    for (pos, text) in label_pos.into_iter().zip(labels) {
        crate::svg::label::draw_edge_label(&mut svg, pos, text, theme);
    }

    // Nodes.
    for (i, node) in d.nodes.iter().enumerate() {
        let center = pos[&(i as NodeId)];
        let size = node_sizes[i];
        draw_node(&mut svg, center, size, node, &d.class_defs, theme);
    }

    svg.finish()
}

/// Resolve every edge's drawn geometry once, for both the canvas measurement
/// and the drawing; `None` for an invisible link or an unknown endpoint.
fn route_edges(
    d: &FlowchartDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    node_sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    edge_pts: &HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) -> Vec<Option<EdgePath>> {
    let axes = local_axes(d);
    let ins = d
        .edges
        .iter()
        .enumerate()
        .map(|(ei, fedge)| {
            if fedge.line == EdgeLine::Invisible {
                return None;
            }
            let geom = resolve_edge_geometry(
                fedge, id_to_u32, &d.nodes, node_sizes, pos, boxes, edge_pts,
            )?;
            let style = resolve_edge_style(&d.link_style_default, d.edge_styles.get(&ei));
            // Curve precedence: per-edge `@{ curve }`, then `linkStyle N
            // interpolate`, then `linkStyle default interpolate`, then the
            // `config.flowchart.curve` diagram default, else basis.
            let curve = fedge
                .curve
                .or_else(|| d.edge_interpolate.get(&ei).copied())
                .or(d.default_interpolate)
                .or(d.config_curve)
                .unwrap_or_default();
            let paired = match (id_to_u32.get(&fedge.from), id_to_u32.get(&fedge.to)) {
                (Some(&u), Some(&v)) => u == v || edge_pts.contains_key(&(v, u)),
                _ => false,
            };
            let horizontal = edge_horizontal(&axes, d.direction, &fedge.from, &fedge.to);
            Some(EdgeIn {
                geom,
                edge: fedge,
                curve,
                style,
                horizontal,
                paired,
            })
        })
        .collect();
    let obstacles: Vec<Obstacle> = d
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            Some(Obstacle {
                id: &n.id,
                node: EndClip {
                    center: *pos.get(&(i as NodeId))?,
                    size: *node_sizes.get(i)?,
                    shape: Some(n.shape),
                },
            })
        })
        .collect();
    let mut frames: Vec<[f64; 4]> = boxes.values().map(|b| [b.0, b.1, b.2, b.3]).collect();
    frames.sort_by(|a, b| {
        a.iter()
            .zip(b)
            .fold(std::cmp::Ordering::Equal, |o, (x, y)| {
                o.then(x.total_cmp(y))
            })
    });
    route_all(ins, &obstacles, &frames)
}

#[cfg(test)]
mod tests;
