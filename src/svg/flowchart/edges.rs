//! Flowchart edge routing, endpoint clipping, drawing, and marker defs.

use std::collections::HashMap;

use crate::parse::{EdgeCurve, EdgeHead, EdgeLine, FlowEdge, FlowNode, NodeShape};
use crate::sugiyama::NodeId;

use crate::svg::builder::{curve_basis_path, curve_linear_path, curve_step_path, fnum, SvgBuilder};
use crate::svg::geometry::{clip_circle, clip_rect, clip_rhombus};
use crate::svg::theme::Theme;

use super::route::{EdgePath, Pt, ALIGNED_EPS};

/// Clip target for one end of an edge: the shape boundary a connector stops at.
#[derive(Clone, Copy)]
pub(super) struct EndClip {
    pub(super) center: (f64, f64),
    pub(super) size: (f64, f64),
    /// `None` marks a subgraph cluster box (clipped as a rectangle).
    pub(super) shape: Option<NodeShape>,
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

/// An edge's resolved clip-target endpoints plus its unclipped waypoint
/// polyline.
pub(super) type EdgeGeometry = (EndClip, EndClip, Vec<(f64, f64)>);

/// Resolves one edge's clip-target endpoints and its unclipped waypoint
/// polyline (the routed layout points, or a straight fallback between
/// endpoint centers). Shared by the flowchart renderer's canvas-sizing
/// measurement pass and its real drawing pass so both compute geometry
/// identically.
pub(super) fn resolve_edge_geometry(
    fedge: &FlowEdge,
    id_to_u32: &HashMap<String, NodeId>,
    nodes: &[FlowNode],
    node_sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    edge_pts: &HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) -> Option<EdgeGeometry> {
    let start = endpoint_clip(&fedge.from, id_to_u32, nodes, node_sizes, pos, boxes)?;
    let end = endpoint_clip(&fedge.to, id_to_u32, nodes, node_sizes, pos, boxes)?;
    let pts: Vec<(f64, f64)> = match (id_to_u32.get(&fedge.from), id_to_u32.get(&fedge.to)) {
        (Some(&u), Some(&v)) => match edge_pts.get(&(u, v)) {
            Some(p) if p.len() >= 2 => p.clone(),
            _ => vec![start.center, end.center],
        },
        _ => vec![start.center, end.center],
    };
    Some((start, end, pts))
}

// ---- edge drawing ----------------------------------------------------------

/// Draws the edge's path; invisible links (`~~~`) only shape the layout.
pub(super) fn draw_edge(svg: &mut SvgBuilder, path: &EdgePath, edge: &FlowEdge, theme: &Theme) {
    if edge.line == EdgeLine::Invisible {
        return;
    }
    let d = match path.curve {
        EdgeCurve::Linear => curve_linear_path(&path.ctrl),
        EdgeCurve::Step => curve_step_path(&path.ctrl),
        _ => curve_basis_path(&path.ctrl),
    };
    let style_override = &path.style;

    // linkStyle overrides layer over the kind-based defaults.
    let stroke = style_override
        .stroke
        .as_deref()
        .unwrap_or(&theme.flow_edge_stroke);
    let default_w = match edge.line {
        EdgeLine::Thick => "3",
        _ => "1.5",
    };
    let width = style_override.stroke_width.as_deref().unwrap_or(default_w);
    // An animated edge needs a dash pattern to flow; keep any explicit/dotted
    // dash, otherwise fall back to a plain `8 8`.
    let dash_val = match (&style_override.stroke_dasharray, edge.line) {
        (Some(da), _) => Some(da.clone()),
        (None, EdgeLine::Dotted) => Some("2 4".to_string()),
        _ => None,
    };
    let dash_val = if edge.animate {
        dash_val.or_else(|| Some("8 8".to_string()))
    } else {
        dash_val
    };
    let dash = dash_val
        .as_ref()
        .map(|d| format!(" stroke-dasharray=\"{d}\""))
        .unwrap_or_default();
    let marker = marker_attr(edge_marker(edge.tail), edge_marker(edge.head));
    let attrs =
        format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{width}\"{dash} {marker}");
    if edge.animate {
        // SMIL animation on stroke-dashoffset: works in browsers without JS.
        // The offset sweeps one full dash period so the flow loops seamlessly.
        let period = dash_period(dash_val.as_deref().unwrap_or("8 8"));
        let anim = format!(
            "<animate attributeName=\"stroke-dashoffset\" values=\"{};0\" \
             dur=\"0.6s\" repeatCount=\"indefinite\"/>",
            fnum(period)
        );
        svg.raw(&format!("<path d=\"{d}\" {attrs}>{anim}</path>"));
    } else {
        svg.path(&d, &attrs);
    }
}

/// Upstream renders every edge through `curveBasis`, so a bare two-point edge
/// between offset nodes leaves and re-enters along its dominant axis as a
/// gentle S rather than a straight diagonal. Only the plain (clip-first)
/// route uses this; aligned ends stay collinear and draw straight.
pub(super) fn basis_control_points(pts: &[Pt]) -> Vec<Pt> {
    if pts.len() != 2 {
        return pts.to_vec();
    }
    let ((ax, ay), (bx, by)) = (pts[0], pts[1]);
    let (dx, dy) = (bx - ax, by - ay);
    if dy.abs() >= dx.abs() {
        if dx.abs() < ALIGNED_EPS {
            return pts.to_vec();
        }
        let my = (ay + by) / 2.0;
        vec![(ax, ay), (ax, my), (bx, my), (bx, by)]
    } else {
        if dy.abs() < ALIGNED_EPS {
            return pts.to_vec();
        }
        let mx = (ax + bx) / 2.0;
        vec![(ax, ay), (mx, ay), (mx, by), (bx, by)]
    }
}

/// Sum the numeric lengths in a `stroke-dasharray` value (e.g. `"8 8"` → 16) to
/// get the offset the animation must travel for one seamless loop.
fn dash_period(dash: &str) -> f64 {
    let sum: f64 = dash
        .split([' ', ','])
        .filter_map(|t| t.trim().parse::<f64>().ok())
        .sum();
    if sum > 0.0 {
        sum
    } else {
        16.0
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

// ---- shape clipping --------------------------------------------------------

pub(super) fn clip_end(from: (f64, f64), clip: &EndClip) -> (f64, f64) {
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
    let flow_edge_stroke = &theme.flow_edge_stroke;
    // Slightly larger than the 10px default to match upstream's arrowheads (#331).
    svg.def_arrow_marker("arrow-filled", flow_edge_stroke, 10, 12);
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

impl EndClip {
    pub(super) fn center_on(&self, axis: usize) -> f64 {
        super::route::axes(self.center)[axis]
    }
}

/// How far a marker reaches back along the edge from its tip, per unit of
/// stroke width (markers scale with the stroke), from the defs here.
pub(super) fn marker_reach(head: EdgeHead) -> Option<f64> {
    match head {
        EdgeHead::Arrow => Some(12.0),
        EdgeHead::Circle => Some(10.0),
        EdgeHead::Cross => Some(5.0),
        _ => None,
    }
}
