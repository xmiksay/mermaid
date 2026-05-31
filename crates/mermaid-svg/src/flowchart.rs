//! Flowchart renderer: maps the AST to a `sugiyama::Graph`, runs layered
//! layout, then draws shapes and clipped polyline edges.

use std::collections::HashMap;
use std::fmt::Write as _;

use mermaid_parse::{
    EdgeKind, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram, NodeShape,
};
use sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use crate::svg::{escape, fnum, SvgBuilder};
use crate::theme::{
    FG, FLOW_EDGE_STROKE, FLOW_LABEL_BG, FLOW_NODE_FILL, FLOW_NODE_STROKE,
};

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 20.0;
const PAD_X: f64 = 18.0;
const PAD_Y: f64 = 12.0;
const MIN_W: f64 = 60.0;
const MIN_H: f64 = 40.0;
const CANVAS_PAD: f64 = 20.0;

pub(crate) fn render(d: &FlowchartDiagram) -> String {
    if d.nodes.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0);
        define_markers(&mut svg);
        return svg.finish();
    }

    let dir = d.direction;
    let node_sizes: Vec<(f64, f64)> = d.nodes.iter().map(node_size).collect();
    let id_to_u32: HashMap<String, NodeId> =
        d.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i as NodeId)).collect();

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
    let cfg = LayoutConfig::default();
    let layout = layout_with(&g, &cfg).unwrap_or_default();

    let (raw_w, raw_h) = (layout.width, layout.height);
    let (canvas_w, canvas_h) = match dir {
        FlowDirection::TopDown | FlowDirection::BottomTop => (raw_w, raw_h),
        FlowDirection::LeftRight | FlowDirection::RightLeft => (raw_h, raw_w),
    };

    let width = canvas_w + CANVAS_PAD * 2.0;
    let height = canvas_h + CANVAS_PAD * 2.0;

    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (tx + CANVAS_PAD, ty + CANVAS_PAD)
    };

    let mut svg = SvgBuilder::new(width, height);
    define_markers(&mut svg);

    // Edges first (so node fills overlay arrow endpoints cleanly).
    for fedge in &d.edges {
        let (Some(&u), Some(&v)) = (id_to_u32.get(&fedge.from), id_to_u32.get(&fedge.to)) else {
            continue;
        };
        let Some(raw_pts) = layout.edge_points.get(&(u, v)) else {
            continue;
        };
        if raw_pts.len() < 2 {
            continue;
        }
        let pts: Vec<(f64, f64)> = raw_pts.iter().map(|&p| transform(p)).collect();
        draw_edge(&mut svg, &pts, fedge, &d.nodes, &id_to_u32, &node_sizes);
    }

    // Nodes
    for (i, node) in d.nodes.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        let size = node_sizes[i];
        draw_node(&mut svg, center, size, node);
    }

    svg.finish()
}

// ---- node drawing ----------------------------------------------------------

fn node_size(n: &FlowNode) -> (f64, f64) {
    let lines: Vec<&str> = n.text.split("\\n").collect();
    let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = (max_chars as f64 * CHAR_W + PAD_X * 2.0).max(MIN_W);
    let h = (lines.len() as f64 * LINE_H + PAD_Y * 2.0).max(MIN_H);
    match n.shape {
        NodeShape::Circle => {
            let d = w.max(h);
            (d, d)
        }
        NodeShape::Rhombus | NodeShape::Hexagon => (w + PAD_X, h + PAD_Y),
        _ => (w, h),
    }
}

fn draw_node(svg: &mut SvgBuilder, (cx, cy): (f64, f64), (w, h): (f64, f64), node: &FlowNode) {
    let fill_attr = format!(
        "fill=\"{FLOW_NODE_FILL}\" stroke=\"{FLOW_NODE_STROKE}\" stroke-width=\"1.5\""
    );
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    match node.shape {
        NodeShape::Rect => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"4\""));
        }
        NodeShape::Round | NodeShape::Stadium => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"{}\"", h / 2.0));
        }
        NodeShape::Subroutine => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"2\""));
            // Inner vertical lines
            svg.line(
                x + 6.0,
                y,
                x + 6.0,
                y + h,
                &format!("stroke=\"{FLOW_NODE_STROKE}\" stroke-width=\"1\" fill=\"none\""),
            );
            svg.line(
                x + w - 6.0,
                y,
                x + w - 6.0,
                y + h,
                &format!("stroke=\"{FLOW_NODE_STROKE}\" stroke-width=\"1\" fill=\"none\""),
            );
        }
        NodeShape::Cylinder => {
            // Body
            svg.rect(x, y + 8.0, w, h - 16.0, &fill_attr);
            // Top ellipse
            svg.path(
                &format!(
                    "M{} {} A{} {} 0 0 0 {} {} A{} {} 0 0 0 {} {}",
                    fnum(x),
                    fnum(y + 8.0),
                    fnum(w / 2.0),
                    fnum(8.0),
                    fnum(x + w),
                    fnum(y + 8.0),
                    fnum(w / 2.0),
                    fnum(8.0),
                    fnum(x),
                    fnum(y + 8.0)
                ),
                &fill_attr,
            );
            // Bottom arc
            svg.path(
                &format!(
                    "M{} {} A{} {} 0 0 0 {} {}",
                    fnum(x),
                    fnum(y + h - 8.0),
                    fnum(w / 2.0),
                    fnum(8.0),
                    fnum(x + w),
                    fnum(y + h - 8.0)
                ),
                &format!("fill=\"none\" stroke=\"{FLOW_NODE_STROKE}\" stroke-width=\"1.5\""),
            );
        }
        NodeShape::Circle => {
            let r = w.max(h) / 2.0;
            svg.circle(cx, cy, r, &fill_attr);
        }
        NodeShape::Rhombus => {
            let d = format!(
                "M{cx} {top}L{right} {cy}L{cx} {bot}L{left} {cy}Z",
                cx = fnum(cx),
                top = fnum(cy - h / 2.0),
                right = fnum(cx + w / 2.0),
                bot = fnum(cy + h / 2.0),
                left = fnum(cx - w / 2.0)
            );
            svg.path(&d, &fill_attr);
        }
        NodeShape::Hexagon => {
            let off = h / 4.0;
            let d = format!(
                "M{x1} {cy}L{x2} {top}L{x3} {top}L{x4} {cy}L{x3} {bot}L{x2} {bot}Z",
                x1 = fnum(x),
                x2 = fnum(x + off),
                x3 = fnum(x + w - off),
                x4 = fnum(x + w),
                top = fnum(cy - h / 2.0),
                bot = fnum(cy + h / 2.0),
                cy = fnum(cy)
            );
            svg.path(&d, &fill_attr);
        }
    }
    draw_label(svg, (cx, cy), &node.text);
}

fn draw_label(svg: &mut SvgBuilder, (cx, cy): (f64, f64), text: &str) {
    let lines: Vec<&str> = text.split("\\n").collect();
    let n = lines.len() as f64;
    let line_h = 18.0;
    let y0 = cy - ((n - 1.0) * line_h) / 2.0 + 5.0;
    for (i, line) in lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * line_h,
            &format!("text-anchor=\"middle\" fill=\"{FG}\""),
            line,
        );
    }
}

// ---- edge drawing ----------------------------------------------------------

fn draw_edge(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    edge: &FlowEdge,
    nodes: &[FlowNode],
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
) {
    let n = pts.len();
    let src_idx = id_to_u32[&edge.from] as usize;
    let dst_idx = id_to_u32[&edge.to] as usize;

    // Clip endpoints to node boundaries.
    let first = clip_to_node(pts[1], pts[0], sizes[src_idx], nodes[src_idx].shape);
    let last = clip_to_node(pts[n - 2], pts[n - 1], sizes[dst_idx], nodes[dst_idx].shape);

    let mut clipped: Vec<(f64, f64)> = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let d = polyline_path(&clipped);
    let (style, marker) = edge_style(edge.kind);
    let attrs = format!(
        "fill=\"none\" stroke=\"{FLOW_EDGE_STROKE}\" {style} {marker}",
        marker = marker_attr(marker)
    );
    svg.path(&d, &attrs);

    if let Some(label) = &edge.label {
        let mid = midpoint(&clipped);
        draw_edge_label(svg, mid, label);
    }
}

fn edge_style(kind: EdgeKind) -> (&'static str, Option<&'static str>) {
    match kind {
        EdgeKind::Solid => ("stroke-width=\"1.5\"", Some("arrow-filled")),
        EdgeKind::SolidNoArrow => ("stroke-width=\"1.5\"", None),
        EdgeKind::Dotted => (
            "stroke-width=\"1.5\" stroke-dasharray=\"2 4\"",
            Some("arrow-filled"),
        ),
        EdgeKind::Thick => ("stroke-width=\"3\"", Some("arrow-filled")),
    }
}

fn marker_attr(m: Option<&str>) -> String {
    match m {
        Some(id) => format!("marker-end=\"url(#{id})\""),
        None => String::new(),
    }
}

fn polyline_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}

fn midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return pts[0];
    }
    // Compute total length, then walk to halfway.
    let mut segs: Vec<f64> = Vec::with_capacity(pts.len() - 1);
    let mut total = 0.0;
    for w in pts.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let l = (dx * dx + dy * dy).sqrt();
        segs.push(l);
        total += l;
    }
    let half = total / 2.0;
    let mut walked = 0.0;
    for (i, w) in pts.windows(2).enumerate() {
        if walked + segs[i] >= half {
            let t = (half - walked) / segs[i].max(1e-9);
            return (w[0].0 + t * (w[1].0 - w[0].0), w[0].1 + t * (w[1].1 - w[0].1));
        }
        walked += segs[i];
    }
    pts[pts.len() / 2]
}

fn draw_edge_label(svg: &mut SvgBuilder, (mx, my): (f64, f64), text: &str) {
    let chars = text.chars().count() as f64;
    let w = chars * 7.0 + 8.0;
    let h = 18.0;
    svg.rect(
        mx - w / 2.0,
        my - h / 2.0,
        w,
        h,
        &format!("fill=\"{FLOW_LABEL_BG}\" stroke=\"none\""),
    );
    svg.text(
        mx,
        my + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{FG}\" font-size=\"12\""),
        text,
    );
}

// ---- shape clipping --------------------------------------------------------

fn clip_to_node(
    from: (f64, f64),
    center: (f64, f64),
    size: (f64, f64),
    shape: NodeShape,
) -> (f64, f64) {
    match shape {
        NodeShape::Circle => clip_circle(from, center, size.0.max(size.1) / 2.0),
        NodeShape::Rhombus => clip_rhombus(from, center, size),
        _ => clip_rect(from, center, size),
    }
}

fn clip_rect(from: (f64, f64), center: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let tx = if dx.abs() > 1e-9 { hw / dx.abs() } else { f64::INFINITY };
    let ty = if dy.abs() > 1e-9 { hh / dy.abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    (center.0 + dx * t, center.1 + dy * t)
}

fn clip_circle(from: (f64, f64), center: (f64, f64), r: f64) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    let d = (dx * dx + dy * dy).sqrt().max(1e-9);
    (center.0 + dx * r / d, center.1 + dy * r / d)
}

fn clip_rhombus(from: (f64, f64), center: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    // Rhombus edge: |x|/hw + |y|/hh = 1
    let t = 1.0 / (dx.abs() / hw + dy.abs() / hh).max(1e-9);
    (center.0 + dx * t, center.1 + dy * t)
}

// ---- markers ---------------------------------------------------------------

fn define_markers(svg: &mut SvgBuilder) {
    let m = format!(
        "<marker id=\"arrow-filled\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" \
         markerWidth=\"10\" markerHeight=\"10\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L10 5 L0 10 z\" fill=\"{FLOW_EDGE_STROKE}\"/></marker>"
    );
    svg.defs_raw(&m);
}

// keep escape in scope for future use (e.g., multi-line tspan)
#[allow(dead_code)]
fn _use_escape(s: &str) -> String {
    escape(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_parse::parse;

    fn parse_flow(s: &str) -> FlowchartDiagram {
        match parse(s).unwrap() {
            mermaid_parse::Diagram::Flowchart(f) => f,
            _ => panic!("expected flowchart"),
        }
    }

    #[test]
    fn renders_basic_td() {
        let d = parse_flow("flowchart TD\nA --> B --> C\n");
        let svg = render(&d);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // Three node rects and two edges (paths).
        assert!(svg.contains("A"));
        assert!(svg.contains("B"));
        assert!(svg.contains("C"));
    }

    #[test]
    fn lr_swaps_dimensions() {
        let d_td = parse_flow("flowchart TD\nA --> B\n");
        let d_lr = parse_flow("flowchart LR\nA --> B\n");
        let svg_td = render(&d_td);
        let svg_lr = render(&d_lr);
        // Extract width/height attributes from each.
        let extract = |s: &str, attr: &str| -> f64 {
            let needle = format!("{attr}=\"");
            let i = s.find(&needle).unwrap() + needle.len();
            let j = s[i..].find('"').unwrap();
            s[i..i + j].parse().unwrap()
        };
        let (tw, th) = (extract(&svg_td, "width"), extract(&svg_td, "height"));
        let (lw, lh) = (extract(&svg_lr, "width"), extract(&svg_lr, "height"));
        // For a two-node chain, TD is taller-than-wide; LR is wider-than-tall.
        assert!(th > tw);
        assert!(lw > lh);
    }

    #[test]
    fn edge_label_appears() {
        let d = parse_flow("flowchart TD\nA -->|yes| B\n");
        let svg = render(&d);
        assert!(svg.contains(">yes<"));
    }

    #[test]
    fn dotted_edge_uses_dasharray() {
        let d = parse_flow("flowchart TD\nA -.-> B\n");
        let svg = render(&d);
        assert!(svg.contains("stroke-dasharray=\"2 4\""));
    }

    #[test]
    fn solid_no_arrow_omits_marker() {
        let d = parse_flow("flowchart TD\nA --- B\n");
        let svg = render(&d);
        // The arrow marker is defined in <defs> but should not be used as marker-end
        // on the only edge.
        // Count occurrences of marker-end usage:
        let usages = svg.matches("marker-end=").count();
        assert_eq!(usages, 0);
    }

    #[test]
    fn empty_flowchart_still_valid_svg() {
        let d = FlowchartDiagram::default();
        let svg = render(&d);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
