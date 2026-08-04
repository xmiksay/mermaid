//! Flowchart node sizing/drawing and subgraph frame rendering.

use std::collections::{HashMap, HashSet};

use crate::parse::{FlowNode, FlowchartDiagram, NodeShape, Style, Subgraph};
use crate::sugiyama::NodeId;

use crate::svg::builder::{fnum, split_label_lines, SvgBuilder};
use crate::svg::interact::{close_click, open_click};
use crate::svg::markup::strip_tags;
use crate::svg::metrics::{font_scale, text_width};
use crate::svg::style::resolve_style;
use crate::svg::theme::Theme;

use super::{CHAR_W, LINE_H, MIN_H, MIN_W, PAD_X, PAD_Y, SUBGRAPH_PAD};

// ---- node sizing & drawing -------------------------------------------------

pub(super) fn node_size(n: &FlowNode, font_size: f64) -> (f64, f64) {
    let lines = split_label_lines(&n.text);
    let widest = lines
        .iter()
        .map(|l| text_width(&strip_tags(l), CHAR_W, font_size))
        .fold(0.0_f64, f64::max);
    let w = (widest + PAD_X * 2.0).max(MIN_W);
    let h = (lines.len() as f64 * LINE_H * font_scale(font_size) + PAD_Y * 2.0).max(MIN_H);
    match n.shape {
        NodeShape::Circle
        | NodeShape::DoubleCircle
        | NodeShape::FilledCircle
        | NodeShape::CrossedCircle => {
            let d = w.max(h);
            (d, d)
        }
        NodeShape::Rhombus | NodeShape::Hexagon => (w + PAD_X, h + PAD_Y),
        NodeShape::Parallelogram
        | NodeShape::ParallelogramAlt
        | NodeShape::Trapezoid
        | NodeShape::TrapezoidAlt => (w + 24.0, h),
        NodeShape::Asymmetric => (w + 16.0, h),
        // Triangles taper, so the label needs more room to sit inside them.
        NodeShape::Triangle | NodeShape::FlippedTriangle => (w + h, h + PAD_Y),
        NodeShape::Hourglass => (w, h + PAD_Y),
        // End-cap ellipses eat horizontal room.
        NodeShape::DirectAccessStorage => (w + 24.0, h),
        _ => (w, h),
    }
}

pub(super) fn draw_node(
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
    let flow_node_stroke = rs.stroke_or(&theme.flow_node_stroke);
    let fill_attr = rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5");
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let off = 12.0; // skew for parallelogram/trapezoid
    match node.shape {
        NodeShape::Rect => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"4\""));
        }
        NodeShape::Round => {
            // Small corner radius — a rounded rect, not a pill (upstream `()`).
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"5\""));
        }
        NodeShape::Stadium => {
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
            super::shapes::cylinder(svg, x, y, w, h, &fill_attr);
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
            // >  ]  — flag with a concave notch on the LEFT edge and a straight
            // vertical right edge (upstream `rect_left_inv_arrow`).
            let d = format!(
                "M{} {} L{} {} L{} {} L{} {} L{} {} Z",
                fnum(x),
                fnum(y),
                fnum(x + w),
                fnum(y),
                fnum(x + w),
                fnum(y + h),
                fnum(x),
                fnum(y + h),
                fnum(x + off),
                fnum(cy)
            );
            svg.path(&d, &fill_attr);
        }
        // Mermaid v11 `@{ shape: … }` geometries live in the sibling module.
        other => super::shapes::draw(
            svg,
            other,
            (x, y, w, h),
            (cx, cy),
            &fill_attr,
            flow_node_stroke,
        ),
    }
    let fg = rs.label_fill(&theme.fg);
    let font = rs.font_size.as_deref();
    draw_label(svg, (cx, cy), &node.text, fg, font, &rs.text_attrs());
    if let Some(action) = &node.click {
        close_click(svg, action);
    }
}

fn draw_label(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    text: &str,
    fg: &str,
    font_size: Option<&str>,
    extra: &str,
) {
    let fs = match font_size {
        Some(s) => format!(" font-size=\"{s}\""),
        None => String::new(),
    };
    // Route the whole label through one `SvgBuilder::text` call: it stacks the
    // lines as vertically-centered <tspan>s and carries an inline tag opened
    // before a `<br>` across the break (#221). Splitting per line here would
    // reset the open-tag stack at each line boundary.
    svg.text(
        cx,
        cy + 5.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\"{fs}{extra}"),
        text,
    );
}

// ---- subgraph frames -------------------------------------------------------

pub(super) fn collect_node_ids<'a>(
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
pub(super) fn compute_subgraph_boxes(
    d: &FlowchartDiagram,
    id_to_u32: &HashMap<String, crate::sugiyama::NodeId>,
    pos: &HashMap<crate::sugiyama::NodeId, (f64, f64)>,
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

/// Push sibling (non-nested) subgraph frames apart when they collide.
///
/// Sugiyama lays out plain node bounding boxes with a uniform `node_gap`; it
/// has no notion of a subgraph's frame (`SUBGRAPH_PAD` + header band), and a
/// subgraph with a local `direction` gets its members transposed in place
/// *after* layout (see `apply_local_directions`), which can turn a narrow
/// column into a wide row. Either can leave two sibling subgraphs' frames —
/// which only need to clear each other, not their raw member nodes — visually
/// overlapping even though sugiyama itself never placed a node on top of
/// another. Resolve it here, post-layout, by nudging whole clusters apart
/// along the axis with the smaller overlap until no sibling pair collides.
pub(super) fn resolve_subgraph_overlaps(
    d: &FlowchartDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &mut HashMap<NodeId, (f64, f64)>,
    edge_pts: &mut HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
    node_sizes: &[(f64, f64)],
) {
    if d.subgraphs.len() < 2 {
        return;
    }
    let sub_idx_by_id: HashMap<&str, usize> = d
        .subgraphs
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let member_sets: Vec<HashSet<NodeId>> = d
        .subgraphs
        .iter()
        .map(|sub| {
            let mut ids = Vec::new();
            collect_node_ids(sub, &d.subgraphs, &sub_idx_by_id, &mut ids);
            ids.iter()
                .filter_map(|id| id_to_u32.get(*id).copied())
                .collect()
        })
        .collect();

    const MARGIN: f64 = 12.0;
    const MAX_PASSES: usize = 8;
    for _ in 0..MAX_PASSES {
        let boxes = compute_subgraph_boxes(d, id_to_u32, pos, node_sizes);
        let mut moved = false;
        for i in 0..d.subgraphs.len() {
            for j in (i + 1)..d.subgraphs.len() {
                if member_sets[i].is_empty() || member_sets[j].is_empty() {
                    continue;
                }
                // Containment is expected and drawn as nesting, not a collision.
                if member_sets[i].is_subset(&member_sets[j])
                    || member_sets[j].is_subset(&member_sets[i])
                {
                    continue;
                }
                let (Some(&(ax0, ay0, ax1, ay1)), Some(&(bx0, by0, bx1, by1))) =
                    (boxes.get(&d.subgraphs[i].id), boxes.get(&d.subgraphs[j].id))
                else {
                    continue;
                };
                let overlap_x = ax1.min(bx1) - ax0.max(bx0);
                let overlap_y = ay1.min(by1) - ay0.max(by0);
                if overlap_x <= 0.0 || overlap_y <= 0.0 {
                    continue;
                }
                moved = true;
                if overlap_x <= overlap_y {
                    let shift = overlap_x / 2.0 + MARGIN / 2.0;
                    let sign = if ax0 + ax1 <= bx0 + bx1 { -1.0 } else { 1.0 };
                    translate_cluster(&member_sets[i], sign * shift, 0.0, pos, edge_pts);
                    translate_cluster(&member_sets[j], -sign * shift, 0.0, pos, edge_pts);
                } else {
                    let shift = overlap_y / 2.0 + MARGIN / 2.0;
                    let sign = if ay0 + ay1 <= by0 + by1 { -1.0 } else { 1.0 };
                    translate_cluster(&member_sets[i], 0.0, sign * shift, pos, edge_pts);
                    translate_cluster(&member_sets[j], 0.0, -sign * shift, pos, edge_pts);
                }
            }
        }
        if !moved {
            break;
        }
    }

    // Any edge with exactly one endpoint in a moved cluster no longer has a
    // meaningful routed polyline (its old bend points were computed against
    // pre-shift positions); fall back to a straight line between the new
    // endpoints, matching `apply_local_directions`'s handling of the same case.
    let all_members: HashSet<NodeId> = member_sets.iter().flatten().copied().collect();
    for (&(a, b), pts) in edge_pts.iter_mut() {
        if all_members.contains(&a) != all_members.contains(&b) {
            if let (Some(&pa), Some(&pb)) = (pos.get(&a), pos.get(&b)) {
                *pts = vec![pa, pb];
            }
        }
    }
}

fn translate_cluster(
    members: &HashSet<NodeId>,
    dx: f64,
    dy: f64,
    pos: &mut HashMap<NodeId, (f64, f64)>,
    edge_pts: &mut HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) {
    for &u in members {
        if let Some(p) = pos.get_mut(&u) {
            p.0 += dx;
            p.1 += dy;
        }
    }
    for (&(a, b), pts) in edge_pts.iter_mut() {
        if members.contains(&a) && members.contains(&b) {
            for p in pts.iter_mut() {
                p.0 += dx;
                p.1 += dy;
            }
        }
    }
}

pub(super) fn draw_subgraphs(
    svg: &mut SvgBuilder,
    d: &FlowchartDiagram,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    theme: &Theme,
) {
    let fg = &theme.fg;
    for sub in &d.subgraphs {
        let Some(&(x0, y0, x1, y1)) = boxes.get(&sub.id) else {
            continue;
        };
        // Themed cluster fill + solid border, overridable by a `style`/`class`
        // on the subgraph id (upstream styles the cluster rect).
        let rs = resolve_style(&d.class_defs, &sub.classes, &sub.style);
        let frame = rs.shape_attrs(&theme.flow_cluster_fill, &theme.flow_cluster_stroke, "1");
        svg.rect(x0, y0, x1 - x0, y1 - y0, &format!("{frame} rx=\"6\""));
        let label = if sub.label.is_empty() {
            sub.id.as_str()
        } else {
            sub.label.as_str()
        };
        let label_fill = rs.label_fill(fg);
        // Upstream draws the cluster title in normal weight, set inside the frame
        // below the top border rather than bold sitting on it (#331).
        svg.text(
            (x0 + x1) / 2.0,
            y0 + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{label_fill}\" font-size=\"13\""),
            label,
        );
    }
}
