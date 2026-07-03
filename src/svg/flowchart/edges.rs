//! Flowchart edge routing, endpoint clipping, drawing, and marker defs, plus
//! per-subgraph local-direction transposition.

use std::collections::{HashMap, HashSet};

use crate::parse::{
    EdgeHead, EdgeLine, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram, NodeShape,
};
use crate::sugiyama::NodeId;

use crate::svg::builder::{curve_basis_path, SvgBuilder};
use crate::svg::geometry::{clip_circle, clip_rect, clip_rhombus, polyline_midpoint};
use crate::svg::style::ResolvedStyle;
use crate::svg::theme::Theme;

/// Clip target for one end of an edge: the shape boundary a connector stops at.
pub(super) struct EndClip {
    pub(super) center: (f64, f64),
    size: (f64, f64),
    /// `None` marks a subgraph cluster box (clipped as a rectangle).
    shape: Option<NodeShape>,
}

/// Resolve an edge endpoint id to its clip target — a node boundary if it names
/// a node, otherwise the bounding box of the subgraph it names.
pub(super) fn endpoint_clip(
    id: &str,
    id_to_u32: &HashMap<String, NodeId>,
    nodes: &[FlowNode],
    node_sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
) -> Option<EndClip> {
    if let Some(&u) = id_to_u32.get(id) {
        return Some(EndClip {
            center: pos[&u],
            size: node_sizes[u as usize],
            shape: Some(nodes[u as usize].shape),
        });
    }
    let &(x0, y0, x1, y1) = boxes.get(id)?;
    Some(EndClip {
        center: ((x0 + x1) / 2.0, (y0 + y1) / 2.0),
        size: (x1 - x0, y1 - y0),
        shape: None,
    })
}

fn is_horizontal(d: FlowDirection) -> bool {
    matches!(d, FlowDirection::LeftRight | FlowDirection::RightLeft)
}

/// Apply each subgraph's local `direction` by transposing its members (and
/// their internal edges) in place about the cluster centre. Only clusters whose
/// flow axis differs from the diagram's are affected — a TD chain inside a
/// `direction LR` subgraph becomes a horizontal row, matching upstream.
pub(super) fn apply_local_directions(
    d: &FlowchartDiagram,
    global_dir: FlowDirection,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &mut HashMap<NodeId, (f64, f64)>,
    edge_pts: &mut HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) {
    let mut moved: HashSet<NodeId> = HashSet::new();
    for sub in &d.subgraphs {
        let Some(local) = sub.direction else { continue };
        if is_horizontal(local) == is_horizontal(global_dir) {
            continue;
        }
        let members: Vec<NodeId> = sub
            .node_ids
            .iter()
            .filter_map(|id| id_to_u32.get(id).copied())
            .filter(|u| !moved.contains(u))
            .collect();
        if members.len() < 2 {
            continue;
        }
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for &u in &members {
            let (x, y) = pos[&u];
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        let (cx, cy) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
        let transpose = |(x, y): (f64, f64)| (cx + (y - cy), cy + (x - cx));

        let member_set: HashSet<NodeId> = members.iter().copied().collect();
        for &u in &members {
            let p = transpose(pos[&u]);
            pos.insert(u, p);
            moved.insert(u);
        }
        for (&(a, b), pts) in edge_pts.iter_mut() {
            let (a_in, b_in) = (member_set.contains(&a), member_set.contains(&b));
            if a_in && b_in {
                for p in pts.iter_mut() {
                    *p = transpose(*p);
                }
            } else if a_in || b_in {
                *pts = vec![pos[&a], pos[&b]];
            }
        }
    }
}

// ---- edge drawing ----------------------------------------------------------

pub(super) fn draw_edge(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    edge: &FlowEdge,
    style_override: &ResolvedStyle,
    start: &EndClip,
    end: &EndClip,
    theme: &Theme,
) {
    // Invisible links (`~~~`) only shape the layout; they draw nothing.
    if edge.line == EdgeLine::Invisible {
        return;
    }
    let n = pts.len();
    let first = clip_end(pts[1], start);
    let last = clip_end(pts[n - 2], end);

    let mut clipped: Vec<(f64, f64)> = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let d = curve_basis_path(&clipped);

    // linkStyle overrides layer over the kind-based defaults.
    let stroke = style_override
        .stroke
        .as_deref()
        .unwrap_or(theme.flow_edge_stroke);
    let default_w = match edge.line {
        EdgeLine::Thick => "3",
        _ => "1.5",
    };
    let width = style_override.stroke_width.as_deref().unwrap_or(default_w);
    let dash = match (&style_override.stroke_dasharray, edge.line) {
        (Some(da), _) => format!(" stroke-dasharray=\"{da}\""),
        (None, EdgeLine::Dotted) => " stroke-dasharray=\"2 4\"".to_string(),
        _ => String::new(),
    };
    let marker = marker_attr(edge_marker(edge.tail), edge_marker(edge.head));
    let attrs =
        format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{width}\"{dash} {marker}");
    svg.path(&d, &attrs);

    if let Some(label) = &edge.label {
        let mid = polyline_midpoint(&clipped);
        draw_edge_label(svg, mid, label, theme);
    }
}

fn edge_marker(head: EdgeHead) -> Option<&'static str> {
    match head {
        EdgeHead::None => None,
        EdgeHead::Arrow => Some("arrow-filled"),
        EdgeHead::Circle => Some("arrow-circle"),
        EdgeHead::Cross => Some("arrow-cross"),
    }
}

fn marker_attr(start: Option<&str>, end: Option<&str>) -> String {
    // The markers use `orient="auto-start-reverse"`, so the same id flips to
    // point outward when referenced as `marker-start` (the cross is symmetric).
    let mut attrs = Vec::new();
    if let Some(id) = start {
        attrs.push(format!("marker-start=\"url(#{id})\""));
    }
    if let Some(id) = end {
        attrs.push(format!("marker-end=\"url(#{id})\""));
    }
    attrs.join(" ")
}

fn draw_edge_label(svg: &mut SvgBuilder, (mx, my): (f64, f64), text: &str, theme: &Theme) {
    let fg = theme.fg;
    let flow_label_bg = theme.flow_label_bg;
    let w = crate::svg::metrics::text_width(text, 7.0, theme.font_size) + 8.0;
    let h = 18.0;
    svg.rect(
        mx - w / 2.0,
        my - h / 2.0,
        w,
        h,
        &format!("fill=\"{flow_label_bg}\" stroke=\"none\""),
    );
    svg.text(
        mx,
        my + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
        text,
    );
}

// ---- shape clipping --------------------------------------------------------

fn clip_end(from: (f64, f64), clip: &EndClip) -> (f64, f64) {
    match clip.shape {
        Some(shape) => clip_to_node(from, clip.center, clip.size, shape),
        None => clip_rect(from, clip.center, clip.size),
    }
}

fn clip_to_node(
    from: (f64, f64),
    center: (f64, f64),
    size: (f64, f64),
    shape: NodeShape,
) -> (f64, f64) {
    match shape {
        NodeShape::Circle
        | NodeShape::DoubleCircle
        | NodeShape::FilledCircle
        | NodeShape::CrossedCircle => clip_circle(from, center, size.0.max(size.1) / 2.0),
        NodeShape::Rhombus => clip_rhombus(from, center, size),
        // Every other v11 geometry falls back to the rectangle-boundary clip.
        _ => clip_rect(from, center, size),
    }
}

// ---- markers ---------------------------------------------------------------

pub(super) fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
    let flow_edge_stroke = theme.flow_edge_stroke;
    svg.defs_raw(&format!(
        "<marker id=\"arrow-filled\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" \
         markerWidth=\"10\" markerHeight=\"10\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L10 5 L0 10 z\" fill=\"{flow_edge_stroke}\"/></marker>"
    ));
    svg.defs_raw(&format!(
        "<marker id=\"arrow-circle\" viewBox=\"0 0 12 12\" refX=\"10\" refY=\"6\" \
         markerWidth=\"12\" markerHeight=\"12\" orient=\"auto-start-reverse\">\
         <circle cx=\"6\" cy=\"6\" r=\"5\" fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"/></marker>"
    ));
    svg.defs_raw(&format!(
        "<marker id=\"arrow-cross\" viewBox=\"0 0 10 10\" refX=\"5\" refY=\"5\" \
         markerWidth=\"10\" markerHeight=\"10\" orient=\"auto\">\
         <path d=\"M1 1 L9 9 M9 1 L1 9\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"/></marker>"
    ));
}
