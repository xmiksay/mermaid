//! Flowchart node sizing/drawing and subgraph frame rendering.

use std::collections::HashMap;

use crate::parse::{FlowNode, FlowchartDiagram, NodeShape, Style, Subgraph};

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
    let flow_node_stroke = rs.stroke_or(theme.flow_node_stroke);
    let fill_attr = rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5");
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

pub(super) fn draw_subgraphs(
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
        // Themed cluster fill + solid border, overridable by a `style`/`class`
        // on the subgraph id (upstream styles the cluster rect).
        let rs = resolve_style(&d.class_defs, &sub.classes, &sub.style);
        let frame = rs.shape_attrs(theme.flow_cluster_fill, theme.flow_cluster_stroke, "1");
        svg.rect(x0, y0, x1 - x0, y1 - y0, &format!("{frame} rx=\"6\""));
        let label = if sub.label.is_empty() {
            sub.id.as_str()
        } else {
            sub.label.as_str()
        };
        let label_fill = rs.label_fill(fg);
        svg.text(
            (x0 + x1) / 2.0,
            y0 + 15.0,
            &format!("text-anchor=\"middle\" fill=\"{label_fill}\" font-size=\"13\" font-weight=\"bold\""),
            label,
        );
    }
}
