//! Class diagram renderer. Boxes with three compartments (name, attributes,
//! methods), connected by relationship lines whose markers depend on kind.

use std::collections::HashMap;

use crate::parse::{ClassDiagram, ClassNote, FlowDirection, MemberKind, Style, UmlClass};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{split_label_lines, SvgBuilder};
use super::geometry::clip_rect;
use super::interact::{close_click, open_click};
use super::metrics::font_scale;
use super::style::resolve_style;
use super::theme::Theme;

mod members;
mod namespace;
mod relations;

use members::{convert_generics, member_display};
use relations::{define_markers, draw_relation};

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 18.0;
const PAD_X: f64 = 14.0;
const HEADER_PAD: f64 = 24.0;
const COMPARTMENT_PAD: f64 = 8.0;
const MIN_W: f64 = 110.0;
const CANVAS_PAD: f64 = 24.0;

pub(crate) fn render(d: &ClassDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    if d.classes.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0).theme(theme);
        define_markers(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d
        .classes
        .iter()
        .map(|c| class_size(c, theme.font_size))
        .collect();
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

    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (tx + CANVAS_PAD, ty + CANVAS_PAD)
    };

    // Work in screen space so namespace containment can be enforced after
    // layout: build node positions and edge polylines, then push any class
    // declared outside a namespace clear of that namespace's frame.
    let mut pos: HashMap<NodeId, (f64, f64)> = (0..d.classes.len() as NodeId)
        .map(|u| (u, transform(layout.node_pos[&u])))
        .collect();
    let mut edge_pts: HashMap<(NodeId, NodeId), Vec<(f64, f64)>> = layout
        .edge_points
        .iter()
        .map(|(k, v)| (*k, v.iter().map(|&p| transform(p)).collect()))
        .collect();

    let frames = namespace::frames(d, &id_to_u32, &pos, &sizes);
    namespace::separate_outsiders(d, &id_to_u32, &sizes, &frames, &mut pos, &mut edge_pts);

    // A left-side push can carry a node past the canvas edge; translate
    // everything right so nothing is clipped, then recompute frames on the
    // final positions.
    let min_left = pos
        .iter()
        .map(|(u, &(x, _))| x - sizes[*u as usize].0 / 2.0)
        .chain(frames.iter().map(|f| f.x))
        .fold(f64::INFINITY, f64::min);
    if min_left < 0.0 {
        let shift = -min_left;
        for p in pos.values_mut() {
            p.0 += shift;
        }
        for v in edge_pts.values_mut() {
            for p in v.iter_mut() {
                p.0 += shift;
            }
        }
    }
    let frames = namespace::frames(d, &id_to_u32, &pos, &sizes);

    let mut width = canvas_w + CANVAS_PAD * 2.0;
    let mut height = canvas_h + CANVAS_PAD * 2.0;
    for (u, &(x, _)) in &pos {
        width = width.max(x + sizes[*u as usize].0 / 2.0 + CANVAS_PAD);
    }
    for f in &frames {
        width = width.max(f.x + f.w + CANVAS_PAD);
    }

    // Notes are laid out in a row below the diagram body; grow the canvas to fit.
    let mut notes: Vec<NoteBox> = Vec::new();
    if !d.notes.is_empty() {
        let mut nx = CANVAS_PAD;
        let ny = height + NOTE_GAP;
        let mut row_h: f64 = 0.0;
        for note in &d.notes {
            let (nw, nh) = note_size(&note.text, theme.font_size);
            notes.push(NoteBox {
                note,
                x: nx,
                y: ny,
                w: nw,
                h: nh,
            });
            nx += nw + NOTE_GAP;
            row_h = row_h.max(nh);
        }
        width = width.max(nx - NOTE_GAP + CANVAS_PAD);
        height = ny + row_h + CANVAS_PAD;
    }

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    define_markers(&mut svg, theme);

    // Relations first.
    for rel in &d.relations {
        let (Some(&u), Some(&v)) = (id_to_u32.get(&rel.from), id_to_u32.get(&rel.to)) else {
            continue;
        };
        let Some(pts) = edge_pts.get(&(u, v)) else {
            continue;
        };
        if pts.len() < 2 {
            continue;
        }
        draw_relation(&mut svg, pts, rel, &sizes, &id_to_u32, theme);
    }

    // Classes.
    for (i, c) in d.classes.iter().enumerate() {
        let center = pos[&(i as NodeId)];
        draw_class(&mut svg, center, sizes[i], c, &d.class_defs, theme);
    }

    // Namespace frames around their member classes. A nested namespace's
    // classes count toward its ancestors' bounds too (the parser registers each
    // class with every enclosing namespace), so a deeper namespace draws a
    // smaller frame nested inside its parent's.
    for f in &frames {
        let ns = &d.namespaces[f.idx];
        svg.rect(
            f.x,
            f.y,
            f.w,
            f.h,
            "fill=\"none\" stroke=\"#888\" stroke-width=\"1\" rx=\"4\" stroke-dasharray=\"4 3\"",
        );
        svg.text(
            f.x + 8.0,
            f.y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-style=\"italic\""),
            ns.label.as_deref().unwrap_or(&ns.name),
        );
    }

    // Notes (yellow sticky boxes) with a dashed connector to their target class.
    for nb in &notes {
        if let Some(target) = &nb.note.target {
            if let Some(&u) = id_to_u32.get(target) {
                let center = pos[&u];
                let anchor = (nb.x + nb.w / 2.0, nb.y);
                let end = clip_rect(anchor, center, sizes[u as usize]);
                svg.line(
                    anchor.0,
                    anchor.1,
                    end.0,
                    end.1,
                    &format!("stroke=\"{fg}\" stroke-width=\"1\" stroke-dasharray=\"4 3\""),
                );
            }
        }
        draw_note(&mut svg, nb, theme);
    }

    svg.finish()
}

const NOTE_GAP: f64 = 18.0;
const NOTE_PAD: f64 = 8.0;

struct NoteBox<'a> {
    note: &'a ClassNote,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn note_size(text: &str, font_size: f64) -> (f64, f64) {
    let lines = split_label_lines(text);
    let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = (max_chars as f64 * CHAR_W * font_scale(font_size) + NOTE_PAD * 2.0).max(60.0);
    let h = lines.len().max(1) as f64 * LINE_H + NOTE_PAD * 2.0;
    (w, h)
}

fn draw_note(svg: &mut SvgBuilder, nb: &NoteBox, theme: &Theme) {
    let fg = &theme.fg;
    svg.rect(
        nb.x,
        nb.y,
        nb.w,
        nb.h,
        "fill=\"#fff5ad\" stroke=\"#aaaa33\" stroke-width=\"1\" rx=\"2\"",
    );
    let lines = split_label_lines(&nb.note.text);
    let mut y = nb.y + NOTE_PAD + LINE_H - 5.0;
    for line in lines {
        svg.text(
            nb.x + NOTE_PAD,
            y,
            &format!("fill=\"{fg}\" font-size=\"13\""),
            line,
        );
        y += LINE_H;
    }
}

/// The header text drawn in a class box: the explicit `["label"]` if any, else
/// the class name with generic `~T~` converted to `<T>`.
fn class_display(c: &UmlClass) -> String {
    c.label.clone().unwrap_or_else(|| convert_generics(&c.name))
}

fn class_size(c: &UmlClass, font_size: f64) -> (f64, f64) {
    let mut max_chars = class_display(c).chars().count();
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
    let w = (max_chars as f64 * CHAR_W * font_scale(font_size) + PAD_X * 2.0).max(MIN_W);
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
    if let Some(action) = &c.click {
        open_click(svg, action);
    }
    let rs = resolve_style(class_defs, &c.classes, &c.style);
    let fg = rs.label_fill(&theme.fg);
    let flow_node_stroke = rs.stroke_or(&theme.flow_node_stroke);
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let base = rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5");
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
        &class_display(c),
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

    if let Some(action) = &c.click {
        close_click(svg, action);
    }
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
    fn style_applies_to_class_box() {
        let d = build("classDiagram\nAnimal --> Dog\nstyle Animal fill:#abc\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#abc\""));
    }

    #[test]
    fn class_label_renders_without_brackets() {
        let d = build("classDiagram\nclass Animal[\"Animal with a label\"]\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("Animal with a label"));
        // No brackets and no duplicate bare-name box.
        assert!(!svg.contains('['));
        assert!(!svg.contains(']'));
    }

    #[test]
    fn note_renders_yellow_box() {
        let d = build("classDiagram\nclass Duck\nnote for Duck \"can fly\"\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("can fly"));
        assert!(svg.contains("#fff5ad"));
        // Dashed connector to the target class.
        assert!(svg.contains("stroke-dasharray=\"4 3\""));
    }

    #[test]
    fn click_wraps_class_in_anchor() {
        let d = build("classDiagram\nclass Shape\nclick Shape href \"https://example.com\"\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("<a href=\"https://example.com\">"));
    }

    #[test]
    fn nested_namespace_draws_both_frames_with_labels() {
        let d = build(
            "classDiagram\n\
             namespace Outer[\"Outer Label\"] {\n\
             class A\n\
             namespace Inner {\n\
             class B\n\
             }\n\
             }\n",
        );
        let svg = render(&d, &Theme::default());
        // Both frames render; the outer shows its bracket label, not raw text.
        assert!(svg.contains("Outer Label"));
        assert!(svg.contains(">Inner<"));
        assert!(!svg.contains('['));
        // Two dashed namespace frames are drawn.
        assert!(svg.matches("stroke-dasharray=\"4 3\"").count() >= 2);
    }

    /// Every `<rect .../>` as `(x, y, w, h, attrs)`.
    fn rects(svg: &str) -> Vec<(f64, f64, f64, f64, String)> {
        svg.match_indices("<rect ")
            .map(|(i, _)| {
                let tag = &svg[i..i + svg[i..].find("/>").unwrap()];
                let get = |k: &str| -> f64 {
                    let start = tag.find(&format!("{k}=\"")).unwrap() + k.len() + 2;
                    let end = tag[start..].find('"').unwrap();
                    tag[start..start + end].parse().unwrap()
                };
                (
                    get("x"),
                    get("y"),
                    get("width"),
                    get("height"),
                    tag.to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn outsider_class_does_not_straddle_namespace_border() {
        // `Cat` is declared outside `namespace Domain`; its box must land fully
        // clear of the dashed frame, never straddling the border (issue #249).
        let d = build(
            "classDiagram\n\
             namespace Domain {\n\
             class Animal\n\
             class Dog\n\
             }\n\
             class Cat\n\
             Animal <|-- Dog\n\
             Animal <|-- Cat\n",
        );
        let svg = render(&d, &Theme::default());
        let all = rects(&svg);
        let frame = all
            .iter()
            .find(|(.., attrs)| attrs.contains("stroke=\"#888\""))
            .expect("namespace frame drawn");
        let (fx, fy, fw, fh, _) = frame;
        let (fx1, fy1) = (fx + fw, fy + fh);

        for (x, y, w, h, attrs) in all.iter().filter(|(.., a)| a.contains("rx=\"2\"")) {
            let (x1, y1) = (x + w, y + h);
            let overlaps = *x < fx1 && x1 > *fx && *y < fy1 && y1 > *fy;
            let inside = *x >= *fx && x1 <= fx1 && *y >= *fy && y1 <= fy1;
            // A class box either sits fully inside the frame (member) or fully
            // clear of it (outsider) — it must never straddle the border.
            assert!(
                !overlaps || inside,
                "class box ({x},{y},{w},{h}) straddles namespace frame \
                 ({fx},{fy},{fw},{fh}); attrs={attrs}"
            );
        }
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
