//! block-beta edge routing and block-arrow shapes: `arrow_path` (double/shafted
//! arrows), `draw_edge`, and the shape-aware `clip` used to land arrowheads on
//! node boundaries.

use std::collections::BTreeMap;

use crate::parse::{BlockArrow, BlockEdge, BlockLinkStyle, BlockShape, EdgeHead};

use crate::svg::builder::{fnum, SvgBuilder};
use crate::svg::theme::Theme;

use super::Geom;

/// SVG path for a block arrow pointing along the set direction(s). Cardinal
/// singles get a shafted arrow; `(x)`/`(y)` render a double-headed arrow.
pub(super) fn arrow_path(a: BlockArrow, x: f64, y: f64, w: f64, h: f64) -> String {
    let px = |f: f64| fnum(x + w * f);
    let py = |f: f64| fnum(y + h * f);
    let p = |fx: f64, fy: f64| format!("{} {}", px(fx), py(fy));
    let pts: &[(f64, f64)] = if a.left && a.right {
        &[
            (0.0, 0.5),
            (0.2, 0.0),
            (0.2, 0.25),
            (0.8, 0.25),
            (0.8, 0.0),
            (1.0, 0.5),
            (0.8, 1.0),
            (0.8, 0.75),
            (0.2, 0.75),
            (0.2, 1.0),
        ]
    } else if a.up && a.down {
        &[
            (0.5, 0.0),
            (1.0, 0.2),
            (0.75, 0.2),
            (0.75, 0.8),
            (1.0, 0.8),
            (0.5, 1.0),
            (0.0, 0.8),
            (0.25, 0.8),
            (0.25, 0.2),
            (0.0, 0.2),
        ]
    } else if a.left {
        &[
            (1.0, 0.25),
            (0.4, 0.25),
            (0.4, 0.0),
            (0.0, 0.5),
            (0.4, 1.0),
            (0.4, 0.75),
            (1.0, 0.75),
        ]
    } else if a.up {
        &[
            (0.25, 1.0),
            (0.25, 0.4),
            (0.0, 0.4),
            (0.5, 0.0),
            (1.0, 0.4),
            (0.75, 0.4),
            (0.75, 1.0),
        ]
    } else if a.down {
        &[
            (0.25, 0.0),
            (0.25, 0.6),
            (0.0, 0.6),
            (0.5, 1.0),
            (1.0, 0.6),
            (0.75, 0.6),
            (0.75, 0.0),
        ]
    } else {
        // right (default)
        &[
            (0.0, 0.25),
            (0.6, 0.25),
            (0.6, 0.0),
            (1.0, 0.5),
            (0.6, 1.0),
            (0.6, 0.75),
            (0.0, 0.75),
        ]
    };
    let mut d = String::new();
    for (i, (fx, fy)) in pts.iter().enumerate() {
        d.push_str(if i == 0 { "M" } else { "L" });
        d.push_str(&p(*fx, *fy));
    }
    d.push('Z');
    d
}

pub(super) fn draw_edge(
    e: &BlockEdge,
    nodes: &BTreeMap<String, Geom>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    // An invisible link only shapes layout — draw nothing.
    if e.style == BlockLinkStyle::Invisible {
        return;
    }
    let stroke = &theme.flow_edge_stroke;
    let (Some(a), Some(b)) = (nodes.get(&e.from), nodes.get(&e.to)) else {
        return;
    };
    // Clip both ends to the node boundaries so the arrowhead sits on the edge.
    let (ax, ay) = clip((b.cx, b.cy), a);
    let (bx, by) = clip((a.cx, a.cy), b);
    // The markers use `orient="auto-start-reverse"`, so the same id flips to
    // point outward at the tail (`marker-start`) end.
    let mut marker = String::new();
    if let Some(id) = marker_id(e.tail) {
        define_marker(svg, e.tail);
        marker.push_str(&format!(" marker-start=\"url(#{id})\""));
    }
    if let Some(id) = marker_id(e.head) {
        define_marker(svg, e.head);
        marker.push_str(&format!(" marker-end=\"url(#{id})\""));
    }
    let (width, dash) = match e.style {
        BlockLinkStyle::Thick => ("2.6", ""),
        BlockLinkStyle::Dotted => ("1.5", " stroke-dasharray=\"3 3\""),
        _ => ("1.5", ""),
    };
    svg.line(
        ax,
        ay,
        bx,
        by,
        &format!("stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}{marker}"),
    );
    if let Some(label) = &e.label {
        let mid = ((ax + bx) / 2.0, (ay + by) / 2.0);
        crate::svg::label::draw_edge_label(svg, mid, label, theme);
    }
}

/// SVG `<marker>` id for an edge end, or `None` for a plain end (`---`).
fn marker_id(head: EdgeHead) -> Option<&'static str> {
    match head {
        EdgeHead::Arrow => Some("blockarrow"),
        EdgeHead::Circle => Some("blockcircle"),
        EdgeHead::Cross => Some("blockcross"),
        _ => None,
    }
}

/// Emit the `<marker>` definition backing `head` (idempotent per id — the same
/// definition may be appended once per referencing edge).
fn define_marker(svg: &mut SvgBuilder, head: EdgeHead) {
    let def = match head {
        EdgeHead::Arrow => return svg.def_arrow_marker("blockarrow", "#333", 9, 6),
        EdgeHead::Circle => "<marker id=\"blockcircle\" viewBox=\"0 0 12 12\" refX=\"10\" refY=\"6\" markerWidth=\"9\" markerHeight=\"9\" orient=\"auto-start-reverse\"><circle cx=\"6\" cy=\"6\" r=\"5\" fill=\"#fff\" stroke=\"#333\" stroke-width=\"1.5\"/></marker>",
        EdgeHead::Cross => "<marker id=\"blockcross\" viewBox=\"0 0 10 10\" refX=\"5\" refY=\"5\" markerWidth=\"9\" markerHeight=\"9\" orient=\"auto\"><path d=\"M1,1 L9,9 M9,1 L1,9\" stroke=\"#333\" stroke-width=\"1.5\"/></marker>",
        _ => return,
    };
    svg.defs_raw(def);
}

/// Clip the point `from → node.center` to the node's shape boundary.
fn clip(from: (f64, f64), n: &Geom) -> (f64, f64) {
    let center = (n.cx, n.cy);
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    match n.shape {
        Some(BlockShape::Circle) | Some(BlockShape::DoubleCircle) => {
            let r = n.w.min(n.h) / 2.0;
            let d = (dx * dx + dy * dy).sqrt().max(1e-9);
            (center.0 + dx * r / d, center.1 + dy * r / d)
        }
        Some(BlockShape::Rhombus) => {
            let t = 1.0 / (dx.abs() / (n.w / 2.0) + dy.abs() / (n.h / 2.0)).max(1e-9);
            (center.0 + dx * t, center.1 + dy * t)
        }
        _ => {
            let tx = if dx.abs() > 1e-9 {
                (n.w / 2.0) / dx.abs()
            } else {
                f64::INFINITY
            };
            let ty = if dy.abs() > 1e-9 {
                (n.h / 2.0) / dy.abs()
            } else {
                f64::INFINITY
            };
            let t = tx.min(ty);
            (center.0 + dx * t, center.1 + dy * t)
        }
    }
}
