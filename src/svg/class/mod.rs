//! Class diagram renderer. Boxes with three compartments (name, attributes,
//! methods), connected by relationship lines whose markers depend on kind.

use std::collections::HashMap;

use crate::parse::{ClassDiagram, ClassNote, FlowDirection};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{split_label_lines, SvgBuilder};
use super::geometry::clip_rect;
use super::metrics::font_scale;
use super::theme::Theme;

mod boxes;
mod members;
mod namespace;
mod relations;
#[cfg(test)]
mod tests;

use boxes::{class_size, draw_class};
use relations::{define_markers, draw_relation};

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 18.0;
const PAD_X: f64 = 14.0;
const HEADER_PAD: f64 = 24.0;
const COMPARTMENT_PAD: f64 = 8.0;
/// Height reserved for an empty attribute/method compartment. Upstream always
/// draws the three-compartment box (name + attributes + methods), so a
/// memberless class shows two empty rows rather than a single plain rect.
const EMPTY_COMPARTMENT_H: f64 = 20.0;
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

    // A left-side push can carry a node past the canvas edge, and a namespace
    // frame's header band extends above/left of its topmost/leftmost member;
    // translate everything so nothing is clipped, then recompute frames on the
    // final positions.
    let min_left = pos
        .iter()
        .map(|(u, &(x, _))| x - sizes[*u as usize].0 / 2.0)
        .chain(frames.iter().map(|f| f.x))
        .fold(f64::INFINITY, f64::min);
    let min_top = frames.iter().map(|f| f.y).fold(f64::INFINITY, f64::min);
    let shift_x = (-min_left).max(0.0);
    let shift_y = (-min_top).max(0.0);
    if shift_x > 0.0 || shift_y > 0.0 {
        for p in pos.values_mut() {
            p.0 += shift_x;
            p.1 += shift_y;
        }
        for v in edge_pts.values_mut() {
            for p in v.iter_mut() {
                p.0 += shift_x;
                p.1 += shift_y;
            }
        }
    }
    let frames = namespace::frames(d, &id_to_u32, &pos, &sizes);

    let mut width = canvas_w + CANVAS_PAD * 2.0;
    let mut height = canvas_h + CANVAS_PAD * 2.0 + shift_y;
    for (u, &(x, _)) in &pos {
        width = width.max(x + sizes[*u as usize].0 / 2.0 + CANVAS_PAD);
    }
    for f in &frames {
        width = width.max(f.x + f.w + CANVAS_PAD);
        height = height.max(f.y + f.h + CANVAS_PAD);
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

    // Namespace frames first — a solid light-yellow rect with the title centered
    // at the top, matching upstream. Drawn before the relations and class boxes
    // so its fill sits behind them rather than covering them.
    for f in &frames {
        let ns = &d.namespaces[f.idx];
        svg.rect(
            f.x,
            f.y,
            f.w,
            f.h,
            &format!(
                "fill=\"{}\" stroke=\"{}\" stroke-width=\"1\" rx=\"4\"",
                theme.flow_cluster_fill, theme.flow_cluster_stroke
            ),
        );
        svg.text(
            f.x + f.w / 2.0,
            f.y + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            ns.label.as_deref().unwrap_or(&ns.name),
        );
    }

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
