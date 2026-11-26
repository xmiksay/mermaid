//! State diagram renderer. Layout via sugiyama, custom shapes for
//! start/end/choice/fork/join pseudo-states.

use std::collections::{HashMap, HashSet};

use crate::parse::{FlowDirection, StateDiagram, StateTransition};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_points, SvgBuilder};
use super::geometry::fit_edges;
use super::interact::{close_click, open_click};
use super::theme::Theme;

mod composite;
use composite::*;
mod edges;
use edges::*;
mod shapes;
use shapes::*;
#[cfg(test)]
mod tests;

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 20.0;
const PAD_X: f64 = 18.0;
const PAD_Y: f64 = 12.0;
const MIN_W: f64 = 70.0;
const MIN_H: f64 = 40.0;
const PSEUDO_R: f64 = 10.0; // start/end radius
const CANVAS_PAD: f64 = 24.0;
const CHOICE_W: f64 = 60.0;
const CHOICE_H: f64 = 40.0;

pub(crate) fn render(d: &StateDiagram, theme: &Theme) -> String {
    if d.states.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0).theme(theme);
        define_marker(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d
        .states
        .iter()
        .map(|s| state_size(s, theme.font_size))
        .collect();
    let id_to_u32: HashMap<String, NodeId> = d
        .states
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i as NodeId))
        .collect();

    // Composite states are drawn as cluster frames around their members, not as
    // standalone nodes; external transitions clip to the frame the way flowchart
    // subgraphs do. So they are excluded from the layout graph.
    let composite_ids: HashSet<&str> = d.composites.iter().map(|c| c.id.as_str()).collect();

    let nodes: Vec<NodeId> = (0..d.states.len() as NodeId)
        .filter(|&u| !composite_ids.contains(d.states[u as usize].id.as_str()))
        .collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .transitions
        .iter()
        .filter_map(|t| {
            if composite_ids.contains(t.from.as_str()) || composite_ids.contains(t.to.as_str()) {
                return None;
            }
            Some((*id_to_u32.get(&t.from)?, *id_to_u32.get(&t.to)?))
        })
        .collect();
    let node_size_su: HashMap<NodeId, (f64, f64)> = nodes
        .iter()
        .map(|&u| {
            let (w, h) = sizes[u as usize];
            let s = match dir {
                FlowDirection::LeftRight | FlowDirection::RightLeft => (h, w),
                _ => (w, h),
            };
            (u, s)
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

    // Screen-space positions for laid-out (non-composite) states.
    let mut pos: HashMap<NodeId, (f64, f64)> = layout
        .node_pos
        .iter()
        .map(|(&u, &p)| (u, transform(p)))
        .collect();

    // Stack the parallel regions of multi-region composites into disjoint
    // vertical bands; record how far each moved so routed edges follow.
    let orig_pos = pos.clone();
    let mut dividers = stack_regions(d, &id_to_u32, &sizes, &mut pos);
    let node_offset: HashMap<NodeId, (f64, f64)> = pos
        .iter()
        .filter_map(|(&u, &(x, y))| {
            let (ox, oy) = orig_pos[&u];
            let off = (x - ox, y - oy);
            (off.0 != 0.0 || off.1 != 0.0).then_some((u, off))
        })
        .collect();

    let mut boxes = compute_composite_boxes(d, &id_to_u32, &pos, &sizes);

    // Note boxes are text-measured (width depends on content), so their real
    // extent has to be computed up front — same two-pass "measure, then size"
    // as node/composite bounds below — instead of assuming fixed padding.
    let note_boxes: Vec<(f64, f64, f64, f64)> = d
        .notes
        .iter()
        .filter_map(|note| note_box(note, &id_to_u32, &sizes, &pos, &boxes, theme))
        .collect();

    // Canvas extent from node boundaries, cluster frames, and note boxes. A
    // frame reserves header room above its members for the title, so its
    // top/left can fall above/left of the topmost node — measure every
    // corner and shift everything back into a positive CANVAS_PAD margin so
    // nothing is clipped by the viewBox edges (issue #242).
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = 0.0_f64;
    let mut max_y = 0.0_f64;
    for (&u, &(x, y)) in &pos {
        let (w, h) = sizes[u as usize];
        min_x = min_x.min(x - w / 2.0);
        min_y = min_y.min(y - h / 2.0);
        max_x = max_x.max(x + w / 2.0);
        max_y = max_y.max(y + h / 2.0);
    }
    for &(bx0, by0, bx1, by1) in boxes.values().chain(note_boxes.iter()) {
        min_x = min_x.min(bx0);
        min_y = min_y.min(by0);
        max_x = max_x.max(bx1);
        max_y = max_y.max(by1);
    }
    // Routed polylines follow their region's stacking offset (both ends
    // share a region, hence one offset).
    let mut routed: HashMap<(NodeId, NodeId), Vec<(f64, f64)>> = layout
        .edge_points
        .iter()
        .filter(|(_, p)| p.len() >= 2)
        .map(|(&(u, v), p)| {
            let (ox, oy) = node_offset.get(&u).copied().unwrap_or((0.0, 0.0));
            let pts = p
                .iter()
                .map(|&q| transform(q))
                .map(|(x, y)| (x + ox, y + oy));
            ((u, v), pts.collect())
        })
        .collect();
    let shift_x = (CANVAS_PAD - min_x).max(0.0);
    let shift_y = (CANVAS_PAD - min_y).max(0.0);
    type Pts = Vec<(f64, f64)>;
    let line = |tr: &StateTransition,
                pos: &HashMap<NodeId, (f64, f64)>,
                boxes: &HashMap<String, (f64, f64, f64, f64)>,
                routed: &HashMap<(NodeId, NodeId), Pts>| {
        transition_line(tr, &id_to_u32, &d.states, &sizes, pos, boxes, routed)
    };
    let drawn = d
        .transitions
        .iter()
        .filter_map(|tr| line(tr, &pos, &boxes, &routed))
        .flat_map(|l| curve_basis_points(&l));
    let ((ex, ey), (width, height)) = fit_edges(
        drawn.map(|(x, y)| (x + shift_x, y + shift_y)),
        (max_x + shift_x + CANVAS_PAD, max_y + shift_y + CANVAS_PAD),
        CANVAS_PAD,
    );
    let (shift_x, shift_y) = (shift_x + ex, shift_y + ey);
    if shift_x != 0.0 || shift_y != 0.0 {
        for p in pos.values_mut().chain(routed.values_mut().flatten()) {
            p.0 += shift_x;
            p.1 += shift_y;
        }
        for b in boxes.values_mut() {
            b.0 += shift_x;
            b.1 += shift_y;
            b.2 += shift_x;
            b.3 += shift_y;
        }
        for ys in dividers.values_mut() {
            for y in ys {
                *y += shift_y;
            }
        }
    }

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    define_marker(&mut svg, theme);

    // Cluster frames first (under nodes/edges) so labels stay legible.
    draw_composites(&mut svg, d, &boxes, &dividers, theme);

    // Transitions: draw every path first, collecting each labeled one's
    // clipped polyline, then resolve all label positions in one pass so two
    // unrelated transitions (including an opposite pair, `A --> B` alongside
    // `B --> A`, whose curves bow apart but whose labels would otherwise
    // still land at the same midpoint) don't occlude one another (#312).
    let mut label_slots: Vec<super::label_layout::LabelSlot> = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for tr in &d.transitions {
        let Some(clipped) = line(tr, &pos, &boxes, &routed) else {
            continue;
        };
        draw_transition(&mut svg, &clipped, theme);
        if let Some(label) = &tr.label {
            if !clipped.is_empty() {
                let w = crate::svg::metrics::text_width(label, 7.0, theme.font_size) + 8.0;
                label_slots.push(super::label_layout::LabelSlot {
                    polyline: clipped,
                    size: (w, 18.0),
                });
                labels.push(label);
            }
        }
    }
    let label_positions = super::label_layout::layout_edge_labels(&label_slots);
    for (pos, text) in label_positions.into_iter().zip(labels) {
        crate::svg::label::draw_edge_label(&mut svg, pos, text, theme);
    }

    for (i, state) in d.states.iter().enumerate() {
        let Some(&center) = pos.get(&(i as NodeId)) else {
            continue;
        };
        if let Some(action) = &state.click {
            open_click(&mut svg, action);
        }
        draw_state(&mut svg, center, sizes[i], state, &d.class_defs, theme);
        if let Some(action) = &state.click {
            close_click(&mut svg, action);
        }
    }

    // Notes attached to states.
    for note in &d.notes {
        draw_state_note(&mut svg, note, &id_to_u32, &sizes, &pos, &boxes, theme);
    }

    svg.finish()
}

fn define_marker(svg: &mut SvgBuilder, theme: &Theme) {
    svg.def_arrow_marker("state-arrow", &theme.flow_edge_stroke, 10, 10);
}
