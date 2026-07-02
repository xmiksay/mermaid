//! Class diagram renderer. Boxes with three compartments (name, attributes,
//! methods), connected by relationship lines whose markers depend on kind.

use std::collections::HashMap;

use crate::parse::{
    ClassDiagram, ClassRelation, ClassRelationKind, FlowDirection, MemberKind, Style, UmlClass,
    Visibility,
};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_path, escape, SvgBuilder};
use super::style::resolve_style;
use super::theme::Theme;

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 18.0;
const PAD_X: f64 = 14.0;
const HEADER_PAD: f64 = 24.0;
const COMPARTMENT_PAD: f64 = 8.0;
const MIN_W: f64 = 110.0;
const CANVAS_PAD: f64 = 24.0;

pub(crate) fn render(d: &ClassDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    if d.classes.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0).font(theme.font_family, theme.font_size);
        define_markers(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d.classes.iter().map(class_size).collect();
    let id_to_u32: HashMap<String, NodeId> = d
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.clone(), i as NodeId))
        .collect();
    let nodes: Vec<NodeId> = (0..d.classes.len() as NodeId).collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .relations
        .iter()
        .filter_map(|r| Some((*id_to_u32.get(&r.from)?, *id_to_u32.get(&r.to)?)))
        .collect();
    let node_size_su: HashMap<NodeId, (f64, f64)> = d
        .classes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let (w, h) = sizes[i];
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

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);
    define_markers(&mut svg, theme);

    // Relations first.
    for rel in &d.relations {
        let (Some(&u), Some(&v)) = (id_to_u32.get(&rel.from), id_to_u32.get(&rel.to)) else {
            continue;
        };
        let Some(raw_pts) = layout.edge_points.get(&(u, v)) else {
            continue;
        };
        if raw_pts.len() < 2 {
            continue;
        }
        let pts: Vec<(f64, f64)> = raw_pts.iter().map(|&p| transform(p)).collect();
        draw_relation(&mut svg, &pts, rel, &sizes, &id_to_u32, theme);
    }

    // Classes.
    for (i, c) in d.classes.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        draw_class(&mut svg, center, sizes[i], c, &d.class_defs, theme);
    }

    // Namespace frames around their member classes.
    for ns in &d.namespaces {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for name in &ns.class_names {
            let Some(&u) = id_to_u32.get(name) else {
                continue;
            };
            let (cx, cy) = transform(layout.node_pos[&u]);
            let (w, h) = sizes[u as usize];
            min_x = min_x.min(cx - w / 2.0);
            max_x = max_x.max(cx + w / 2.0);
            min_y = min_y.min(cy - h / 2.0);
            max_y = max_y.max(cy + h / 2.0);
        }
        if !min_x.is_finite() {
            continue;
        }
        let pad = 12.0;
        let header_h = 18.0;
        let x = min_x - pad;
        let y = min_y - pad - header_h;
        let w = (max_x - min_x) + pad * 2.0;
        let h = (max_y - min_y) + pad * 2.0 + header_h;
        svg.rect(
            x,
            y,
            w,
            h,
            "fill=\"none\" stroke=\"#888\" stroke-width=\"1\" rx=\"4\" stroke-dasharray=\"4 3\"",
        );
        svg.text(
            x + 8.0,
            y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-style=\"italic\""),
            &ns.name,
        );
    }

    svg.finish()
}

fn class_size(c: &UmlClass) -> (f64, f64) {
    let mut max_chars = convert_generics(&c.name).chars().count();
    if let Some(s) = &c.stereotype {
        max_chars = max_chars.max(s.chars().count() + 4);
    }
    let attr_lines = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Attribute)
        .count();
    let meth_lines = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Method)
        .count();
    for m in &c.members {
        let len = member_display(m).text.chars().count();
        if len > max_chars {
            max_chars = len;
        }
    }
    let w = (max_chars as f64 * CHAR_W + PAD_X * 2.0).max(MIN_W);
    let header_h = if c.stereotype.is_some() {
        HEADER_PAD + LINE_H
    } else {
        HEADER_PAD
    };
    let attr_h = if attr_lines == 0 {
        0.0
    } else {
        attr_lines as f64 * LINE_H + COMPARTMENT_PAD * 2.0
    };
    let meth_h = if meth_lines == 0 {
        0.0
    } else {
        meth_lines as f64 * LINE_H + COMPARTMENT_PAD * 2.0
    };
    let h = header_h + attr_h + meth_h + 4.0;
    (w, h)
}

/// A member ready to draw: its display text (visibility marker + generics
/// converted to angle brackets, classifier suffix stripped) plus the styling
/// the classifier implies — `*` (abstract) → italic, `$` (static) → underline.
struct MemberDisplay {
    text: String,
    is_abstract: bool,
    is_static: bool,
}

fn member_display(m: &crate::parse::ClassMember) -> MemberDisplay {
    let vis = match m.visibility {
        Visibility::Public => "+",
        Visibility::Private => "-",
        Visibility::Protected => "#",
        Visibility::Package => "~",
        Visibility::Default => "",
    };
    let (body, is_abstract, is_static) = split_classifier(m.text.trim());
    MemberDisplay {
        text: format!("{vis}{}", convert_generics(body.trim())),
        is_abstract,
        is_static,
    }
}

/// Strip a trailing UML classifier: `*` marks an abstract member, `$` a static
/// one. Returns the text without the classifier and the two flags.
fn split_classifier(text: &str) -> (&str, bool, bool) {
    match text.chars().last() {
        Some('*') => (&text[..text.len() - 1], true, false),
        Some('$') => (&text[..text.len() - 1], false, true),
        _ => (text, false, false),
    }
}

/// Convert Mermaid generic syntax `~T~` to `<T>`, innermost pair first so that
/// nested generics like `List~List~int~~` become `List<List<int>>` and
/// comma-separated ones like `Map~string, int~` become `Map<string, int>`. A
/// lone unmatched `~` is left untouched.
fn convert_generics(s: &str) -> String {
    let mut s = s.to_string();
    loop {
        let tildes: Vec<usize> = s.match_indices('~').map(|(i, _)| i).collect();
        // Innermost pair = the adjacent tilde pair with non-empty content and
        // the largest opening index; replacing it first unwinds nesting.
        let chosen = tildes
            .windows(2)
            .filter(|w| w[1] > w[0] + 1)
            .map(|w| (w[0], w[1]))
            .next_back();
        let Some((a, b)) = chosen else { break };
        s = format!("{}<{}>{}", &s[..a], &s[a + 1..b], &s[b + 1..]);
    }
    s
}

fn draw_member(svg: &mut SvgBuilder, x: f64, y: f64, m: &crate::parse::ClassMember, fg: &str) {
    let md = member_display(m);
    let mut attrs = format!("fill=\"{fg}\" font-size=\"13\"");
    if md.is_abstract {
        attrs.push_str(" font-style=\"italic\"");
    }
    if md.is_static {
        attrs.push_str(" text-decoration=\"underline\"");
    }
    svg.text(x, y, &attrs, &md.text);
}

fn draw_class(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    (w, h): (f64, f64),
    c: &UmlClass,
    class_defs: &HashMap<String, Style>,
    theme: &Theme,
) {
    let rs = resolve_style(class_defs, &c.classes, &c.style);
    let fg = rs.label_fill(theme.fg);
    let flow_node_stroke = rs.stroke_or(theme.flow_node_stroke);
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let base = rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5");
    svg.rect(x, y, w, h, &format!("{base} rx=\"2\""));

    let mut cursor = y;
    // Header (with optional stereotype line above the name).
    if let Some(s) = &c.stereotype {
        cursor += 16.0;
        svg.text(
            cx,
            cursor,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" font-style=\"italic\""),
            &format!("«{s}»"),
        );
    } else {
        cursor += 6.0;
    }
    cursor += LINE_H;
    svg.text(
        cx,
        cursor,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-weight=\"bold\""),
        &convert_generics(&c.name),
    );
    cursor += 4.0;

    let attrs: Vec<_> = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Attribute)
        .collect();
    let meths: Vec<_> = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Method)
        .collect();

    if !attrs.is_empty() {
        cursor += 4.0;
        svg.line(
            x,
            cursor,
            x + w,
            cursor,
            &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
        );
        cursor += COMPARTMENT_PAD;
        for m in attrs {
            cursor += LINE_H - 4.0;
            draw_member(svg, x + 8.0, cursor, m, fg);
            cursor += 4.0;
        }
        cursor += COMPARTMENT_PAD - 4.0;
    }

    if !meths.is_empty() {
        svg.line(
            x,
            cursor,
            x + w,
            cursor,
            &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
        );
        cursor += COMPARTMENT_PAD;
        for m in meths {
            cursor += LINE_H - 4.0;
            draw_member(svg, x + 8.0, cursor, m, fg);
            cursor += 4.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_relation(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    rel: &ClassRelation,
    sizes: &[(f64, f64)],
    id_to_u32: &HashMap<String, NodeId>,
    theme: &Theme,
) {
    let fg = theme.fg;
    let flow_edge_stroke = theme.flow_edge_stroke;
    let flow_label_bg = theme.flow_label_bg;
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

    let (dash, marker_end, marker_start) = style_for(rel.kind, rel.reversed);
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
        let mid = midpoint(&clipped);
        let chars = label.chars().count() as f64;
        let w = chars * 7.0 + 8.0;
        let h = 16.0;
        svg.rect(
            mid.0 - w / 2.0,
            mid.1 - h / 2.0,
            w,
            h,
            &format!("fill=\"{flow_label_bg}\" stroke=\"none\""),
        );
        svg.text(
            mid.0,
            mid.1 + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            label,
        );
    }
}

/// Draw a small multiplicity label near an edge endpoint. `end` is the point on
/// the node boundary; `toward` is the next waypoint, giving the edge direction.
fn draw_card(svg: &mut SvgBuilder, end: (f64, f64), toward: (f64, f64), text: &str, theme: &Theme) {
    let fg = theme.fg;
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

fn clip_rect(from: (f64, f64), c: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - c.0;
    let dy = from.1 - c.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c;
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
    (c.0 + dx * t, c.1 + dy * t)
}

fn midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return pts[0];
    }
    let mut segs = Vec::with_capacity(pts.len() - 1);
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

fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
    let flow_edge_stroke = theme.flow_edge_stroke;
    // Triangle (hollow) for inheritance/realization — drawn at the parent end
    let triangle = format!(
        "<marker id=\"cls-triangle\" viewBox=\"0 0 12 12\" refX=\"11\" refY=\"6\" \
         markerWidth=\"14\" markerHeight=\"14\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L11 6 L0 12 Z\" fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"/>\
         </marker>"
    );
    let arrow = format!(
        "<marker id=\"cls-arrow\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" \
         markerWidth=\"10\" markerHeight=\"10\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L10 5 L0 10 z\" fill=\"{flow_edge_stroke}\"/></marker>"
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
    svg.defs_raw(&triangle);
    svg.defs_raw(&arrow);
    svg.defs_raw(&diamond_filled);
    svg.defs_raw(&diamond_open);
}

#[allow(dead_code)]
fn _use_escape(s: &str) -> String {
    escape(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> ClassDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Class(c) => c,
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
    fn forward_composition_puts_diamond_at_to_end() {
        // `A --* B`: filled diamond belongs at B (the `to`/whole) via marker-end.
        let d = build("classDiagram\nA --* B\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("marker-end=\"url(#cls-diamond-filled)\""));
        assert!(!svg.contains("marker-start"));
        assert!(!svg.contains("url(#cls-arrow)"));
    }

    #[test]
    fn style_applies_to_class_box() {
        let d = build("classDiagram\nAnimal --> Dog\nstyle Animal fill:#abc\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#abc\""));
    }

    #[test]
    fn generics_convert_to_angle_brackets() {
        assert_eq!(convert_generics("List~int~"), "List<int>");
        assert_eq!(convert_generics("Map~string, int~"), "Map<string, int>");
        assert_eq!(convert_generics("List~List~int~~"), "List<List<int>>");
        // A lone unmatched tilde is left alone.
        assert_eq!(convert_generics("a~b"), "a~b");
    }

    #[test]
    fn generics_render_in_name_and_members() {
        let d = build("classDiagram\nclass List~T~ {\n+items List~int~\n+get() T\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">List&lt;T&gt;<"));
        assert!(svg.contains("items List&lt;int&gt;"));
        assert!(!svg.contains('~'));
    }

    #[test]
    fn abstract_and_static_classifiers_style_members() {
        let d = build("classDiagram\nclass Shape {\n+area() float*\n+count int$\n}\n");
        let svg = render(&d, &Theme::default());
        // Classifier chars are consumed, not rendered.
        assert!(!svg.contains('*'));
        assert!(!svg.contains('$'));
        assert!(svg.contains("font-style=\"italic\""));
        assert!(svg.contains("text-decoration=\"underline\""));
    }

    #[test]
    fn cssclass_applies_classdef() {
        let d = build(
            "classDiagram\nAnimal --> Dog\nclassDef foo fill:#abc\ncssClass \"Animal\" foo\n",
        );
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#abc\""));
    }
}
