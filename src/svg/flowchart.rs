//! Flowchart renderer: maps the AST to a `crate::sugiyama::Graph`, runs layered
//! layout, then draws shapes, clipped polyline edges, and subgraph frames.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::parse::{
    EdgeHead, EdgeLine, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram, NodeShape, Subgraph,
};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{escape, fnum, SvgBuilder};
use super::theme::Theme;

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 20.0;
const PAD_X: f64 = 18.0;
const PAD_Y: f64 = 12.0;
const MIN_W: f64 = 60.0;
const MIN_H: f64 = 40.0;
const CANVAS_PAD: f64 = 24.0;
const SUBGRAPH_PAD: f64 = 16.0;

pub(crate) fn render(d: &FlowchartDiagram, theme: &Theme) -> String {
    if d.nodes.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0);
        define_markers(&mut svg, theme);
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
    let layout = layout_with(&g, &LayoutConfig::default()).unwrap_or_default();

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
    define_markers(&mut svg, theme);

    // Subgraph frames (drawn first so they sit under nodes/edges).
    draw_subgraphs(&mut svg, d, &id_to_u32, &node_sizes, &layout, &transform, theme);

    // Edges.
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
        draw_edge(&mut svg, &pts, fedge, &d.nodes, &id_to_u32, &node_sizes, theme);
    }

    // Nodes.
    for (i, node) in d.nodes.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        let size = node_sizes[i];
        draw_node(&mut svg, center, size, node, theme);
    }

    svg.finish()
}

// ---- node sizing & drawing -------------------------------------------------

fn node_size(n: &FlowNode) -> (f64, f64) {
    let lines: Vec<&str> = n.text.split("\\n").collect();
    let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = (max_chars as f64 * CHAR_W + PAD_X * 2.0).max(MIN_W);
    let h = (lines.len() as f64 * LINE_H + PAD_Y * 2.0).max(MIN_H);
    match n.shape {
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let d = w.max(h);
            (d, d)
        }
        NodeShape::Rhombus | NodeShape::Hexagon => (w + PAD_X, h + PAD_Y),
        NodeShape::Parallelogram
        | NodeShape::ParallelogramAlt
        | NodeShape::Trapezoid
        | NodeShape::TrapezoidAlt => (w + 24.0, h),
        NodeShape::Asymmetric => (w + 16.0, h),
        _ => (w, h),
    }
}

fn draw_node(svg: &mut SvgBuilder, (cx, cy): (f64, f64), (w, h): (f64, f64), node: &FlowNode, theme: &Theme) {
    let flow_node_fill = theme.flow_node_fill;
    let flow_node_stroke = theme.flow_node_stroke;
    let fill_attr = format!(
        "fill=\"{flow_node_fill}\" stroke=\"{flow_node_stroke}\" stroke-width=\"1.5\""
    );
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let off = 12.0; // skew for parallelogram/trapezoid
    match node.shape {
        NodeShape::Rect => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"4\""));
        }
        NodeShape::Round | NodeShape::Stadium => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"{}\"", h / 2.0));
        }
        NodeShape::Subroutine => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"2\""));
            svg.line(
                x + 6.0,
                y,
                x + 6.0,
                y + h,
                &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
            );
            svg.line(
                x + w - 6.0,
                y,
                x + w - 6.0,
                y + h,
                &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
            );
        }
        NodeShape::Cylinder => {
            svg.rect(x, y + 8.0, w, h - 16.0, &fill_attr);
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
                &format!("fill=\"none\" stroke=\"{flow_node_stroke}\" stroke-width=\"1.5\""),
            );
        }
        NodeShape::Circle => {
            let r = w.max(h) / 2.0;
            svg.circle(cx, cy, r, &fill_attr);
        }
        NodeShape::DoubleCircle => {
            let r = w.max(h) / 2.0;
            svg.circle(cx, cy, r, &fill_attr);
            svg.circle(cx, cy, r - 4.0, &fill_attr);
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
            let o = h / 4.0;
            let d = format!(
                "M{x1} {cy}L{x2} {top}L{x3} {top}L{x4} {cy}L{x3} {bot}L{x2} {bot}Z",
                x1 = fnum(x),
                x2 = fnum(x + o),
                x3 = fnum(x + w - o),
                x4 = fnum(x + w),
                top = fnum(cy - h / 2.0),
                bot = fnum(cy + h / 2.0),
                cy = fnum(cy)
            );
            svg.path(&d, &fill_attr);
        }
        NodeShape::Parallelogram => {
            // /  /  — top-left and bottom-right pushed in
            let d = format!(
                "M{} {} L{} {} L{} {} L{} {} Z",
                fnum(x + off),
                fnum(y),
                fnum(x + w),
                fnum(y),
                fnum(x + w - off),
                fnum(y + h),
                fnum(x),
                fnum(y + h)
            );
            svg.path(&d, &fill_attr);
        }
        NodeShape::ParallelogramAlt => {
            // \  \  — top-right and bottom-left pushed in
            let d = format!(
                "M{} {} L{} {} L{} {} L{} {} Z",
                fnum(x),
                fnum(y),
                fnum(x + w - off),
                fnum(y),
                fnum(x + w),
                fnum(y + h),
                fnum(x + off),
                fnum(y + h)
            );
            svg.path(&d, &fill_attr);
        }
        NodeShape::Trapezoid => {
            // /  \  — top narrower than bottom (manual input)
            let d = format!(
                "M{} {} L{} {} L{} {} L{} {} Z",
                fnum(x + off),
                fnum(y),
                fnum(x + w - off),
                fnum(y),
                fnum(x + w),
                fnum(y + h),
                fnum(x),
                fnum(y + h)
            );
            svg.path(&d, &fill_attr);
        }
        NodeShape::TrapezoidAlt => {
            // \  /  — top wider than bottom (manual output)
            let d = format!(
                "M{} {} L{} {} L{} {} L{} {} Z",
                fnum(x),
                fnum(y),
                fnum(x + w),
                fnum(y),
                fnum(x + w - off),
                fnum(y + h),
                fnum(x + off),
                fnum(y + h)
            );
            svg.path(&d, &fill_attr);
        }
        NodeShape::Asymmetric => {
            // >  ]  — arrow flag pointing right
            let d = format!(
                "M{} {} L{} {} L{} {} L{} {} L{} {} Z",
                fnum(x),
                fnum(y),
                fnum(x + w - off),
                fnum(y),
                fnum(x + w),
                fnum(cy),
                fnum(x + w - off),
                fnum(y + h),
                fnum(x),
                fnum(y + h)
            );
            svg.path(&d, &fill_attr);
        }
    }
    draw_label(svg, (cx, cy), &node.text, theme);
}

fn draw_label(svg: &mut SvgBuilder, (cx, cy): (f64, f64), text: &str, theme: &Theme) {
    let fg = theme.fg;
    let lines: Vec<&str> = text.split("\\n").collect();
    let n = lines.len() as f64;
    let line_h = 18.0;
    let y0 = cy - ((n - 1.0) * line_h) / 2.0 + 5.0;
    for (i, line) in lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * line_h,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
    }
}

// ---- subgraph frames -------------------------------------------------------

fn draw_subgraphs(
    svg: &mut SvgBuilder,
    d: &FlowchartDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    node_sizes: &[(f64, f64)],
    layout: &crate::sugiyama::Layout,
    transform: &impl Fn((f64, f64)) -> (f64, f64),
    theme: &Theme,
) {
    let fg = theme.fg;
    // We compute the bounding box of each subgraph by collecting transformed
    // coordinates of all nodes that belong to it (recursively).
    let mut sub_idx_by_id: HashMap<&str, usize> = HashMap::new();
    for (i, s) in d.subgraphs.iter().enumerate() {
        sub_idx_by_id.insert(s.id.as_str(), i);
    }
    fn collect_node_ids<'a>(
        sub: &'a Subgraph,
        all: &'a [Subgraph],
        idx_by_id: &HashMap<&str, usize>,
        out: &mut Vec<&'a str>,
    ) {
        for n in &sub.node_ids {
            out.push(n.as_str());
        }
        for child_id in &sub.child_subgraph_ids {
            if let Some(&i) = idx_by_id.get(child_id.as_str()) {
                collect_node_ids(&all[i], all, idx_by_id, out);
            }
        }
    }

    for sub in &d.subgraphs {
        let mut ids: Vec<&str> = Vec::new();
        collect_node_ids(sub, &d.subgraphs, &sub_idx_by_id, &mut ids);
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for id in &ids {
            let Some(&u) = id_to_u32.get(*id) else {
                continue;
            };
            let (sx, sy) = layout.node_pos[&u];
            let (cx, cy) = transform((sx, sy));
            let (w, h) = node_sizes[u as usize];
            min_x = min_x.min(cx - w / 2.0);
            max_x = max_x.max(cx + w / 2.0);
            min_y = min_y.min(cy - h / 2.0);
            max_y = max_y.max(cy + h / 2.0);
        }
        if !min_x.is_finite() {
            continue;
        }
        let x = min_x - SUBGRAPH_PAD;
        let y = min_y - SUBGRAPH_PAD - 14.0;
        let w = (max_x - min_x) + SUBGRAPH_PAD * 2.0;
        let h = (max_y - min_y) + SUBGRAPH_PAD * 2.0 + 14.0;
        svg.rect(
            x,
            y,
            w,
            h,
            "fill=\"#F8F8FF\" stroke=\"#666\" stroke-width=\"1\" stroke-dasharray=\"6 4\" rx=\"4\"",
        );
        let label = if sub.label.is_empty() {
            sub.id.as_str()
        } else {
            sub.label.as_str()
        };
        svg.text(
            x + 10.0,
            y + 12.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-style=\"italic\""),
            label,
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
    theme: &Theme,
) {
    let flow_edge_stroke = theme.flow_edge_stroke;
    let n = pts.len();
    let src_idx = id_to_u32[&edge.from] as usize;
    let dst_idx = id_to_u32[&edge.to] as usize;

    let first = clip_to_node(pts[1], pts[0], sizes[src_idx], nodes[src_idx].shape);
    let last = clip_to_node(pts[n - 2], pts[n - 1], sizes[dst_idx], nodes[dst_idx].shape);

    let mut clipped: Vec<(f64, f64)> = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let d = polyline_path(&clipped);
    let (style, marker) = edge_style(edge.line, edge.head);
    let attrs = format!(
        "fill=\"none\" stroke=\"{flow_edge_stroke}\" {style} {marker}",
        marker = marker_attr(marker)
    );
    svg.path(&d, &attrs);

    if let Some(label) = &edge.label {
        let mid = midpoint(&clipped);
        draw_edge_label(svg, mid, label, theme);
    }
}

fn edge_style(line: EdgeLine, head: EdgeHead) -> (String, Option<&'static str>) {
    let stroke_width = match line {
        EdgeLine::Thick => "3",
        _ => "1.5",
    };
    let dash = match line {
        EdgeLine::Dotted => " stroke-dasharray=\"2 4\"",
        _ => "",
    };
    let style = format!("stroke-width=\"{stroke_width}\"{dash}");
    let marker = match head {
        EdgeHead::None => None,
        EdgeHead::Arrow => Some("arrow-filled"),
        EdgeHead::Circle => Some("arrow-circle"),
        EdgeHead::Cross => Some("arrow-cross"),
    };
    (style, marker)
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

fn draw_edge_label(svg: &mut SvgBuilder, (mx, my): (f64, f64), text: &str, theme: &Theme) {
    let fg = theme.fg;
    let flow_label_bg = theme.flow_label_bg;
    let chars = text.chars().count() as f64;
    let w = chars * 7.0 + 8.0;
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

fn clip_to_node(
    from: (f64, f64),
    center: (f64, f64),
    size: (f64, f64),
    shape: NodeShape,
) -> (f64, f64) {
    match shape {
        NodeShape::Circle | NodeShape::DoubleCircle => {
            clip_circle(from, center, size.0.max(size.1) / 2.0)
        }
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
    let t = 1.0 / (dx.abs() / hw + dy.abs() / hh).max(1e-9);
    (center.0 + dx * t, center.1 + dy * t)
}

// ---- markers ---------------------------------------------------------------

fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
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

#[allow(dead_code)]
fn _use_escape(s: &str) -> String {
    escape(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn parse_flow(s: &str) -> FlowchartDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Flowchart(f) => f,
            _ => panic!("expected flowchart"),
        }
    }

    #[test]
    fn renders_basic_td() {
        let svg = render(&parse_flow("flowchart TD\nA --> B --> C\n"), &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("A"));
        assert!(svg.contains("C"));
    }

    #[test]
    fn edge_label_appears() {
        let svg = render(&parse_flow("flowchart TD\nA -->|yes| B\n"), &Theme::default());
        assert!(svg.contains(">yes<"));
    }

    #[test]
    fn dotted_edge_uses_dasharray() {
        let svg = render(&parse_flow("flowchart TD\nA -.-> B\n"), &Theme::default());
        assert!(svg.contains("stroke-dasharray=\"2 4\""));
    }

    #[test]
    fn circle_head_marker_used() {
        let svg = render(&parse_flow("flowchart TD\nA --o B\n"), &Theme::default());
        assert!(svg.contains("arrow-circle"));
    }

    #[test]
    fn cross_head_marker_used() {
        let svg = render(&parse_flow("flowchart TD\nA --x B\n"), &Theme::default());
        assert!(svg.contains("arrow-cross"));
    }

    #[test]
    fn solid_no_arrow_omits_marker() {
        let svg = render(&parse_flow("flowchart TD\nA --- B\n"), &Theme::default());
        assert_eq!(svg.matches("marker-end=").count(), 0);
    }

    #[test]
    fn subgraph_frame_drawn() {
        let svg = render(&parse_flow(
            "flowchart TD\nA --> B\nsubgraph S [Group]\nB --> C\nend\n",
        ), &Theme::default());
        // Dashed rect for subgraph + italic label
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains(">Group<"));
    }

    #[test]
    fn all_asymmetric_shapes_render() {
        let svg = render(&parse_flow(
            "flowchart TD\nA[/par/] --> B[\\palt\\]\nB --> C[/trap\\]\nC --> D[\\tralt/]\nD --> E>flag]\n",
        ), &Theme::default());
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn empty_flowchart_still_valid_svg() {
        let svg = render(&FlowchartDiagram::default(), &Theme::default());
        assert!(svg.starts_with("<svg"));
    }
}
