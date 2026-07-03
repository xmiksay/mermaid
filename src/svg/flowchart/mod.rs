//! Flowchart renderer: maps the AST to a `crate::sugiyama::Graph`, runs layered
//! layout, then draws shapes, clipped polyline edges, and subgraph frames.

use std::collections::HashMap;

use crate::parse::{FlowDirection, FlowchartDiagram};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::SvgBuilder;
use super::style::resolve_edge_style;
use super::theme::Theme;

mod edges;
mod nodes;
mod shapes;

use edges::*;
use nodes::*;

pub(super) const CHAR_W: f64 = 7.5;
pub(super) const LINE_H: f64 = 20.0;
pub(super) const PAD_X: f64 = 18.0;
pub(super) const PAD_Y: f64 = 12.0;
pub(super) const MIN_W: f64 = 60.0;
pub(super) const MIN_H: f64 = 40.0;
const CANVAS_PAD: f64 = 24.0;
pub(super) const SUBGRAPH_PAD: f64 = 16.0;
/// Vertical space reserved above the graph for a frontmatter title.
const TITLE_BAND: f64 = 34.0;

pub(crate) fn render(d: &FlowchartDiagram, theme: &Theme) -> String {
    if d.nodes.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0).font(theme.font_family, theme.font_size);
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

    let boxes = compute_subgraph_boxes(d, &id_to_u32, &pos, &node_sizes);

    // Canvas: expand the global extent to include any locally moved nodes and
    // subgraph frames so nothing is clipped by the viewport.
    let mut max_x = 0.0_f64;
    let mut max_y = 0.0_f64;
    for (u, &(x, y)) in &pos {
        let (w, h) = node_sizes[*u as usize];
        max_x = max_x.max(x + w / 2.0);
        max_y = max_y.max(y + h / 2.0);
    }
    for &(_, _, bx1, by1) in boxes.values() {
        max_x = max_x.max(bx1 + SUBGRAPH_PAD);
        max_y = max_y.max(by1 + SUBGRAPH_PAD);
    }
    let mut width = max_x + CANVAS_PAD;
    let height = max_y + CANVAS_PAD;

    // A long title can be wider than the graph itself; grow the canvas to fit.
    if let Some(t) = &d.title {
        let title_w =
            crate::svg::metrics::text_width(t, CHAR_W + 2.0, theme.font_size) + CANVAS_PAD * 2.0;
        width = width.max(title_w);
    }

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);
    define_markers(&mut svg, theme);

    if let Some(t) = &d.title {
        let fg = theme.fg;
        svg.text(
            width / 2.0,
            CANVAS_PAD + 6.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    // Subgraph frames (drawn first so they sit under nodes/edges).
    draw_subgraphs(&mut svg, d, &boxes, theme);

    // Edges.
    for (ei, fedge) in d.edges.iter().enumerate() {
        let edge_style = resolve_edge_style(&d.link_style_default, d.edge_styles.get(&ei));
        // Curve precedence: per-edge `@{ curve }`, then `linkStyle N interpolate`,
        // then `linkStyle default interpolate`, else the default basis.
        let curve = fedge
            .curve
            .or_else(|| d.edge_interpolate.get(&ei).copied())
            .or(d.default_interpolate)
            .unwrap_or_default();
        let (Some(start), Some(end)) = (
            endpoint_clip(&fedge.from, &id_to_u32, &d.nodes, &node_sizes, &pos, &boxes),
            endpoint_clip(&fedge.to, &id_to_u32, &d.nodes, &node_sizes, &pos, &boxes),
        ) else {
            continue;
        };
        // Real node→node edges keep their routed polyline; an endpoint that is
        // a subgraph cluster has no layout route, so draw a straight connector
        // clipped to the cluster box.
        let pts: Vec<(f64, f64)> = match (id_to_u32.get(&fedge.from), id_to_u32.get(&fedge.to)) {
            (Some(&u), Some(&v)) => match edge_pts.get(&(u, v)) {
                Some(p) if p.len() >= 2 => p.clone(),
                _ => vec![start.center, end.center],
            },
            _ => vec![start.center, end.center],
        };
        draw_edge(
            &mut svg,
            &pts,
            fedge,
            curve,
            &edge_style,
            &start,
            &end,
            theme,
        );
    }

    // Nodes.
    for (i, node) in d.nodes.iter().enumerate() {
        let center = pos[&(i as NodeId)];
        let size = node_sizes[i];
        draw_node(&mut svg, center, size, node, &d.class_defs, theme);
    }

    svg.finish()
}

#[cfg(test)]
mod tests;
