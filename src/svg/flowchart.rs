//! Flowchart renderer: maps the AST to a `crate::sugiyama::Graph`, runs layered
//! layout, then draws shapes, clipped polyline edges, and subgraph frames.

use std::collections::{HashMap, HashSet};

use crate::parse::{
    ClickAction, EdgeHead, EdgeLine, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram,
    NodeShape, Style, Subgraph,
};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_path, escape, fnum, split_label_lines, SvgBuilder};
use super::style::{resolve_edge_style, resolve_style, ResolvedStyle};
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
        let mut svg = SvgBuilder::new(40.0, 40.0).font(theme.font_family, theme.font_size);
        define_markers(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    let node_sizes: Vec<(f64, f64)> = d.nodes.iter().map(node_size).collect();
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
        (tx + CANVAS_PAD, ty + CANVAS_PAD)
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
    let width = max_x + CANVAS_PAD;
    let height = max_y + CANVAS_PAD;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);
    define_markers(&mut svg, theme);

    // Subgraph frames (drawn first so they sit under nodes/edges).
    draw_subgraphs(&mut svg, d, &boxes, theme);

    // Edges.
    for (ei, fedge) in d.edges.iter().enumerate() {
        let edge_style = resolve_edge_style(&d.link_style_default, d.edge_styles.get(&ei));
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
        draw_edge(&mut svg, &pts, fedge, &edge_style, &start, &end, theme);
    }

    // Nodes.
    for (i, node) in d.nodes.iter().enumerate() {
        let center = pos[&(i as NodeId)];
        let size = node_sizes[i];
        draw_node(&mut svg, center, size, node, &d.class_defs, theme);
    }

    svg.finish()
}

/// Clip target for one end of an edge: the shape boundary a connector stops at.
struct EndClip {
    center: (f64, f64),
    size: (f64, f64),
    /// `None` marks a subgraph cluster box (clipped as a rectangle).
    shape: Option<NodeShape>,
}

/// Resolve an edge endpoint id to its clip target — a node boundary if it names
/// a node, otherwise the bounding box of the subgraph it names.
fn endpoint_clip(
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
fn apply_local_directions(
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

// ---- node sizing & drawing -------------------------------------------------

fn node_size(n: &FlowNode) -> (f64, f64) {
    let lines = split_label_lines(&n.text);
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

fn draw_node(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    (w, h): (f64, f64),
    node: &FlowNode,
    class_defs: &HashMap<String, Style>,
    theme: &Theme,
) {
    if let Some(action) = &node.click {
        open_click(svg, action);
    }
    let rs = resolve_style(class_defs, &node.classes, &node.style);
    let flow_node_stroke = rs.stroke_or(theme.flow_node_stroke);
    let fill_attr = rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5");
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
    let fg = rs.label_fill(theme.fg);
    let font = rs.font_size.as_deref();
    draw_label(svg, (cx, cy), &node.text, fg, font);
    if let Some(action) = &node.click {
        close_click(svg, action);
    }
}

/// Open the wrapper element for a clickable node: an `<a>` for hyperlinks or a
/// `<g class="clickable" onclick=…>` for JS callbacks, plus a `<title>` tooltip.
fn open_click(svg: &mut SvgBuilder, action: &ClickAction) {
    match action {
        ClickAction::Href {
            url,
            tooltip,
            target,
        } => {
            let target_attr = match target {
                Some(t) => format!(" target=\"{}\"", escape(t)),
                None => String::new(),
            };
            svg.raw(&format!(
                "<a href=\"{url}\"{target_attr}>",
                url = escape(url)
            ));
            emit_tooltip(svg, tooltip);
        }
        ClickAction::Callback { function, tooltip } => {
            let call = if function.contains('(') {
                function.clone()
            } else {
                format!("{function}()")
            };
            svg.raw(&format!(
                "<g class=\"clickable\" style=\"cursor:pointer\" onclick=\"{}\">",
                escape(&call)
            ));
            emit_tooltip(svg, tooltip);
        }
    }
}

fn emit_tooltip(svg: &mut SvgBuilder, tooltip: &Option<String>) {
    if let Some(t) = tooltip {
        svg.raw(&format!("<title>{}</title>", escape(t)));
    }
}

fn close_click(svg: &mut SvgBuilder, action: &ClickAction) {
    match action {
        ClickAction::Href { .. } => svg.raw("</a>"),
        ClickAction::Callback { .. } => svg.raw("</g>"),
    }
}

fn draw_label(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    text: &str,
    fg: &str,
    font_size: Option<&str>,
) {
    let lines = split_label_lines(text);
    let n = lines.len() as f64;
    let line_h = 18.0;
    let y0 = cy - ((n - 1.0) * line_h) / 2.0 + 5.0;
    let fs = match font_size {
        Some(s) => format!(" font-size=\"{s}\""),
        None => String::new(),
    };
    for (i, line) in lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * line_h,
            &format!("text-anchor=\"middle\" fill=\"{fg}\"{fs}"),
            line,
        );
    }
}

// ---- subgraph frames -------------------------------------------------------

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

/// Screen-space bounding box `(x0, y0, x1, y1)` of every subgraph, including the
/// space for its title, keyed by subgraph id. A box spans all member nodes
/// gathered recursively through nested children.
fn compute_subgraph_boxes(
    d: &FlowchartDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &HashMap<NodeId, (f64, f64)>,
    node_sizes: &[(f64, f64)],
) -> HashMap<String, (f64, f64, f64, f64)> {
    let mut sub_idx_by_id: HashMap<&str, usize> = HashMap::new();
    for (i, s) in d.subgraphs.iter().enumerate() {
        sub_idx_by_id.insert(s.id.as_str(), i);
    }
    let mut boxes = HashMap::new();
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
            let (cx, cy) = pos[&u];
            let (w, h) = node_sizes[u as usize];
            min_x = min_x.min(cx - w / 2.0);
            max_x = max_x.max(cx + w / 2.0);
            min_y = min_y.min(cy - h / 2.0);
            max_y = max_y.max(cy + h / 2.0);
        }
        if !min_x.is_finite() {
            continue;
        }
        boxes.insert(
            sub.id.clone(),
            (
                min_x - SUBGRAPH_PAD,
                min_y - SUBGRAPH_PAD - 14.0,
                max_x + SUBGRAPH_PAD,
                max_y + SUBGRAPH_PAD,
            ),
        );
    }
    boxes
}

fn draw_subgraphs(
    svg: &mut SvgBuilder,
    d: &FlowchartDiagram,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    theme: &Theme,
) {
    let fg = theme.fg;
    for sub in &d.subgraphs {
        let Some(&(x0, y0, x1, y1)) = boxes.get(&sub.id) else {
            continue;
        };
        svg.rect(
            x0,
            y0,
            x1 - x0,
            y1 - y0,
            "fill=\"#F8F8FF\" stroke=\"#666\" stroke-width=\"1\" stroke-dasharray=\"6 4\" rx=\"4\"",
        );
        let label = if sub.label.is_empty() {
            sub.id.as_str()
        } else {
            sub.label.as_str()
        };
        svg.text(
            x0 + 10.0,
            y0 + 12.0,
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
    style_override: &ResolvedStyle,
    start: &EndClip,
    end: &EndClip,
    theme: &Theme,
) {
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
        let mid = midpoint(&clipped);
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
            return (
                w[0].0 + t * (w[1].0 - w[0].0),
                w[0].1 + t * (w[1].1 - w[0].1),
            );
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
    let tx = if dx.abs() > 1e-9 {
        hw / dx.abs()
    } else {
        f64::INFINITY
    };
    let ty = if dy.abs() > 1e-9 {
        hh / dy.abs()
    } else {
        f64::INFINITY
    };
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
        let svg = render(
            &parse_flow("flowchart TD\nA --> B --> C\n"),
            &Theme::default(),
        );
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("A"));
        assert!(svg.contains("C"));
    }

    #[test]
    fn edge_label_appears() {
        let svg = render(
            &parse_flow("flowchart TD\nA -->|yes| B\n"),
            &Theme::default(),
        );
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
    fn bidirectional_edge_emits_start_and_end_markers() {
        let svg = render(&parse_flow("flowchart LR\nA <--> B\n"), &Theme::default());
        assert!(svg.contains("marker-start=\"url(#arrow-filled)\""));
        assert!(svg.contains("marker-end=\"url(#arrow-filled)\""));
    }

    #[test]
    fn subgraph_frame_drawn() {
        let svg = render(
            &parse_flow("flowchart TD\nA --> B\nsubgraph S [Group]\nB --> C\nend\n"),
            &Theme::default(),
        );
        // Dashed rect for subgraph + italic label
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains(">Group<"));
    }

    /// Centre `(x, y)` of the single-line node label `id` (`text-anchor
    /// middle` places the `<text>` x at the node's centre).
    fn label_center(svg: &str, id: &str) -> (f64, f64) {
        let needle = format!(">{id}</text>");
        let end = svg.find(&needle).unwrap_or_else(|| panic!("no label {id}"));
        let open = svg[..end].rfind("<text ").unwrap();
        let tag = &svg[open..end];
        let grab = |attr: &str| {
            let s = tag.find(attr).unwrap() + attr.len();
            let e = s + tag[s..].find('"').unwrap();
            tag[s..e].parse::<f64>().unwrap()
        };
        (grab("x=\""), grab("y=\""))
    }

    #[test]
    fn subgraph_local_direction_transposes_members() {
        // Under TD the chain A→B stacks vertically (same x, B below A).
        let td = render(
            &parse_flow("flowchart TD\nsubgraph S\nA --> B\nend\n"),
            &Theme::default(),
        );
        let (ax, ay) = label_center(&td, "A");
        let (bx, by) = label_center(&td, "B");
        assert!((ax - bx).abs() < 1.0, "TD members should share a column");
        assert!(by > ay, "TD flows top-to-bottom");

        // `direction LR` inside the subgraph lays the same members side by side.
        let lr = render(
            &parse_flow("flowchart TD\nsubgraph S\ndirection LR\nA --> B\nend\n"),
            &Theme::default(),
        );
        let (ax, ay) = label_center(&lr, "A");
        let (bx, by) = label_center(&lr, "B");
        assert!((ay - by).abs() < 1.0, "LR members should share a row");
        assert!(bx > ax, "LR flows left-to-right");
    }

    #[test]
    fn edge_to_subgraph_id_routes_to_box() {
        let svg = render(
            &parse_flow("flowchart TD\nsubgraph SG [Group]\nA --> B\nend\nC --> SG\n"),
            &Theme::default(),
        );
        // Cluster frame titled by its label, no phantom `SG` node, and the C→SG
        // edge is drawn (an arrow-headed path) rather than silently dropped.
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains(">Group</text>"));
        assert!(!svg.contains(">SG</text>"));
        assert!(svg.contains("marker-end=\"url(#arrow-filled)\""));
    }

    #[test]
    fn all_asymmetric_shapes_render() {
        let svg = render(&parse_flow(
            "flowchart TD\nA[/par/] --> B[\\palt\\]\nB --> C[/trap\\]\nC --> D[\\tralt/]\nD --> E>flag]\n",
        ), &Theme::default());
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn node_label_br_splits_into_lines() {
        let svg = render(
            &parse_flow("flowchart TB\nPX[\"line one<br/>line two<br/>line three\"]\n"),
            &Theme::default(),
        );
        // Three separate <text> lines, none containing literal <br> markup.
        assert_eq!(svg.matches("line one").count(), 1);
        assert_eq!(svg.matches("line three").count(), 1);
        assert!(!svg.contains("&lt;br"));
        assert!(!svg.contains("<br"));
    }

    #[test]
    fn empty_flowchart_still_valid_svg() {
        let svg = render(&FlowchartDiagram::default(), &Theme::default());
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn inline_style_overrides_theme_fill() {
        let svg = render(
            &parse_flow("flowchart TD\nA --> B\nstyle A fill:#f9f\n"),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"#f9f\""));
    }

    #[test]
    fn classdef_applied_via_class() {
        let svg = render(
            &parse_flow("flowchart TD\nA --> B\nclassDef foo fill:#0f0\nclass A foo\n"),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"#0f0\""));
    }

    #[test]
    fn default_classdef_styles_unclassed_node() {
        let svg = render(
            &parse_flow("flowchart TD\nA --> B\nclassDef default fill:#eee\n"),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"#eee\""));
    }

    #[test]
    fn link_style_overrides_edge_stroke() {
        let svg = render(
            &parse_flow("flowchart TD\nA --> B\nlinkStyle 0 stroke:#ff3,stroke-width:4px\n"),
            &Theme::default(),
        );
        assert!(svg.contains("stroke=\"#ff3\""));
        assert!(svg.contains("stroke-width=\"4\""));
    }

    #[test]
    fn color_prop_sets_label_fill() {
        let svg = render(
            &parse_flow("flowchart TD\nA --> B\nstyle A color:#fff\n"),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"#fff\""));
    }

    /// True if any `<path d="…">` value contains a cubic-bezier `C` command.
    fn any_bezier_path(svg: &str) -> bool {
        svg.split("d=\"").skip(1).any(|seg| {
            let d = &seg[..seg.find('"').unwrap_or(seg.len())];
            d.contains('C')
        })
    }

    #[test]
    fn curved_edges_use_bezier() {
        // The skip edge a→d spans multiple layers, so it routes through ≥3
        // waypoints and emits a cubic-bezier `C` command in its path.
        let svg = render(
            &parse_flow("flowchart TD\na --> b --> c --> d\na --> d\n"),
            &Theme::default(),
        );
        assert!(any_bezier_path(&svg));
    }

    #[test]
    fn click_href_wraps_node_in_anchor() {
        let svg = render(
            &parse_flow("flowchart TD\nA-->B\nclick A \"https://example.com\" \"go\"\n"),
            &Theme::default(),
        );
        assert!(svg.contains("<a href=\"https://example.com\">"));
        assert!(svg.contains("<title>go</title>"));
        assert!(svg.contains("</a>"));
    }

    #[test]
    fn click_href_target_renders_attribute() {
        let svg = render(
            &parse_flow("flowchart TD\nA-->B\nclick A href \"http://x\" \"t\" _blank\n"),
            &Theme::default(),
        );
        assert!(svg.contains("target=\"_blank\""));
    }

    #[test]
    fn click_callback_emits_onclick() {
        let svg = render(
            &parse_flow("flowchart TD\nA-->B\nclick A doThing \"hint\"\n"),
            &Theme::default(),
        );
        assert!(svg.contains("onclick=\"doThing()\""));
        assert!(svg.contains("class=\"clickable\""));
        assert!(svg.contains("<title>hint</title>"));
    }

    #[test]
    fn non_clickable_node_has_no_anchor() {
        let svg = render(&parse_flow("flowchart TD\nA-->B\n"), &Theme::default());
        assert!(!svg.contains("<a "));
        assert!(!svg.contains("onclick"));
    }

    #[test]
    fn adjacent_layer_edge_stays_straight() {
        // A single short edge clips to 2 points → straight M..L.., no curve.
        let svg = render(&parse_flow("flowchart TD\na --> b\n"), &Theme::default());
        assert!(!any_bezier_path(&svg));
    }
}
