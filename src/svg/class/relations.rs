//! Class relationship drawing: edge routing, cardinality labels, and the
//! kind-specific markers (triangle/diamond/arrow) with their orientation.

use std::collections::HashMap;

use crate::parse::{ClassRelation, ClassRelationKind};
use crate::sugiyama::NodeId;

use super::super::builder::{curve_basis_path, SvgBuilder};
use super::super::geometry::{clip_rect, polyline_midpoint};
use super::super::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_relation(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    rel: &ClassRelation,
    sizes: &[(f64, f64)],
    id_to_u32: &HashMap<String, NodeId>,
    theme: &Theme,
) {
    let flow_edge_stroke = &theme.flow_edge_stroke;
    let src = id_to_u32[&rel.from] as usize;
    let dst = id_to_u32[&rel.to] as usize;
    let n = pts.len();
    let first = clip_rect(pts[1], pts[0], sizes[src]);
    let last = clip_rect(pts[n - 2], pts[n - 1], sizes[dst]);

    let mut clipped = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let (dash, mut marker_end, mut marker_start) = style_for(rel.kind, rel.reversed);
    // Two-way relation: `kind` marked the `from` end (reversed), so the mirror
    // marker decorates the `to` end.
    if let Some(to_kind) = rel.to_kind {
        let (_, to_marker, _) = style_for(to_kind, false);
        marker_end = to_marker;
    }
    // A lollipop-interface `()` end draws a socket circle, overriding any
    // kind marker at that end.
    if rel.lollipop_from {
        marker_start = Some("cls-lollipop");
    }
    if rel.lollipop_to {
        marker_end = Some("cls-lollipop");
    }
    let dash_attr = if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{dash}\"")
    };
    let me = match marker_end {
        Some(m) => format!(" marker-end=\"url(#{m})\""),
        None => String::new(),
    };
    let ms = match marker_start {
        Some(m) => format!(" marker-start=\"url(#{m})\""),
        None => String::new(),
    };
    let d = curve_basis_path(&clipped);
    svg.path(
        &d,
        &format!(
            "fill=\"none\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"{dash_attr}{ms}{me}"
        ),
    );

    if let Some(card) = &rel.from_card {
        draw_card(svg, clipped[0], clipped[1], card, theme);
    }
    if let Some(card) = &rel.to_card {
        draw_card(svg, clipped[n - 1], clipped[n - 2], card, theme);
    }

    if let Some(label) = &rel.label {
        let mid = polyline_midpoint(&clipped);
        crate::svg::label::draw_edge_label(svg, mid, label, theme);
    }
}

/// Draw a small multiplicity label near an edge endpoint. `end` is the point on
/// the node boundary; `toward` is the next waypoint, giving the edge direction.
fn draw_card(svg: &mut SvgBuilder, end: (f64, f64), toward: (f64, f64), text: &str, theme: &Theme) {
    let fg = &theme.fg;
    let dx = toward.0 - end.0;
    let dy = toward.1 - end.1;
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (ux, uy) = (dx / len, dy / len);
    // Nudge along the edge away from the box, then perpendicular to clear the line.
    let (px, py) = (-uy, ux);
    let x = end.0 + ux * 14.0 + px * 9.0;
    let y = end.1 + uy * 14.0 + py * 9.0;
    svg.text(
        x,
        y + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
        text,
    );
}

/// Returns `(dash, marker_end, marker_start)`. Each relation kind carries a
/// single decorated marker (triangle/diamond/arrow); composition/aggregation
/// draw only the diamond — no arrowhead at the far end, matching upstream. The
/// marker sits at the `from` end (`marker-start`) for reversed tokens, else at
/// the `to` end (`marker-end`). The markers' `orient="auto-start-reverse"`
/// makes them point into their node at whichever end they land.
fn style_for(
    k: ClassRelationKind,
    reversed: bool,
) -> (&'static str, Option<&'static str>, Option<&'static str>) {
    use ClassRelationKind::*;
    let (dash, marker) = match k {
        Inheritance => ("", Some("cls-triangle")),
        Realization => ("4 3", Some("cls-triangle")),
        Composition => ("", Some("cls-diamond-filled")),
        Aggregation => ("", Some("cls-diamond-open")),
        Association => ("", Some("cls-arrow")),
        Dependency => ("4 3", Some("cls-arrow")),
        Link => ("", None),
        LinkDashed => ("4 3", None),
    };
    if reversed {
        (dash, None, marker)
    } else {
        (dash, marker, None)
    }
}

pub(super) fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
    let flow_edge_stroke = &theme.flow_edge_stroke;
    // Triangle (hollow) for inheritance/realization — drawn at the parent end
    let triangle = format!(
        "<marker id=\"cls-triangle\" viewBox=\"0 0 12 12\" refX=\"11\" refY=\"6\" \
         markerWidth=\"14\" markerHeight=\"14\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L11 6 L0 12 Z\" fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"/>\
         </marker>"
    );
    let diamond_filled = format!(
        "<marker id=\"cls-diamond-filled\" viewBox=\"0 0 16 8\" refX=\"0\" refY=\"4\" \
         markerWidth=\"16\" markerHeight=\"8\" orient=\"auto-start-reverse\">\
         <path d=\"M0 4 L8 0 L16 4 L8 8 Z\" fill=\"{flow_edge_stroke}\" stroke=\"{flow_edge_stroke}\"/>\
         </marker>"
    );
    let diamond_open = format!(
        "<marker id=\"cls-diamond-open\" viewBox=\"0 0 16 8\" refX=\"0\" refY=\"4\" \
         markerWidth=\"16\" markerHeight=\"8\" orient=\"auto-start-reverse\">\
         <path d=\"M0 4 L8 0 L16 4 L8 8 Z\" fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"/>\
         </marker>"
    );
    // Lollipop interface socket — a hollow circle just off the interface end.
    let lollipop = format!(
        "<marker id=\"cls-lollipop\" viewBox=\"0 0 12 12\" refX=\"11\" refY=\"6\" \
         markerWidth=\"14\" markerHeight=\"14\" orient=\"auto-start-reverse\">\
         <circle cx=\"6\" cy=\"6\" r=\"5\" fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"/>\
         </marker>"
    );
    svg.defs_raw(&triangle);
    svg.def_arrow_marker("cls-arrow", flow_edge_stroke, 10, 10);
    svg.defs_raw(&diamond_filled);
    svg.defs_raw(&diamond_open);
    svg.defs_raw(&lollipop);
}

#[cfg(test)]
mod tests {
    use super::super::super::theme::Theme;
    use super::super::render;
    use crate::parse::{parse, ClassDiagram, Diagram};

    fn build(s: &str) -> ClassDiagram {
        match parse(s).unwrap() {
            Diagram::Class(c) => c,
            _ => panic!("not class"),
        }
    }

    #[test]
    fn renders_inheritance() {
        let d = build("classDiagram\nAnimal <|-- Dog\nclass Animal {\n+name\n+eat()\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Animal<"));
        assert!(svg.contains(">Dog<"));
        assert!(svg.contains("cls-triangle"));
    }

    #[test]
    fn cardinality_labels_render_without_corrupting_names() {
        let d = build("classDiagram\nCustomer \"1\" --> \"*\" Order\n");
        let svg = render(&d, &Theme::default());
        // Class names stay clean, and multiplicities appear as their own labels.
        assert!(svg.contains(">Customer<"));
        assert!(svg.contains(">Order<"));
        assert!(!svg.contains("Customer &quot;"));
        assert!(svg.contains(">1<"));
        assert!(svg.contains(">*<"));
    }

    #[test]
    fn composition_has_diamond() {
        let d = build("classDiagram\nCar *-- Wheel\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("cls-diamond-filled"));
        // No spurious arrowhead at the far (non-diamond) end. The marker is
        // still defined in <defs>; it must not be referenced on the edge.
        assert!(!svg.contains("url(#cls-arrow)"));
    }

    #[test]
    fn reversed_inheritance_marks_the_from_end() {
        // `Animal <|-- Dog`: triangle belongs at Animal (the `from`/parent),
        // drawn via marker-start, not marker-end.
        let d = build("classDiagram\nAnimal <|-- Dog\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("marker-start=\"url(#cls-triangle)\""));
        assert!(!svg.contains("marker-end=\"url(#cls-triangle)\""));
    }

    #[test]
    fn forward_inheritance_marks_the_to_end() {
        // `Dog --|> Animal`: triangle at Animal (the `to`/parent) via marker-end.
        let d = build("classDiagram\nDog --|> Animal\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("marker-end=\"url(#cls-triangle)\""));
        assert!(!svg.contains("marker-start=\"url(#cls-triangle)\""));
    }

    #[test]
    fn lollipop_interface_draws_socket_circle() {
        // `bar ()-- foo`: socket circle at the `from` (bar) end via marker-start,
        // and the class name stays clean.
        let d = build("classDiagram\nbar ()-- foo\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">bar<"));
        assert!(!svg.contains("bar ()"));
        assert!(svg.contains("marker-start=\"url(#cls-lollipop)\""));

        // `foo --() baz`: socket circle at the `to` (baz) end via marker-end.
        let d = build("classDiagram\nfoo --() baz\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("marker-end=\"url(#cls-lollipop)\""));
    }

    #[test]
    fn two_way_relation_marks_both_ends() {
        // `Animal <|--|> Zebra`: triangle at both ends, no phantom class.
        let d = build("classDiagram\nAnimal <|--|> Zebra\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">Animal<"));
        assert!(svg.contains(">Zebra<"));
        assert!(svg.contains("marker-start=\"url(#cls-triangle)\""));
        assert!(svg.contains("marker-end=\"url(#cls-triangle)\""));
    }

    #[test]
    fn forward_composition_puts_diamond_at_to_end() {
        // `A --* B`: filled diamond belongs at B (the `to`/whole) via marker-end.
        let d = build("classDiagram\nA --* B\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("marker-end=\"url(#cls-diamond-filled)\""));
        assert!(!svg.contains("marker-start"));
        assert!(!svg.contains("url(#cls-arrow)"));
    }
}
