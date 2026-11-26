//! ER diagram renderer. Entities are drawn as tables (name header + attribute
//! rows), connected by lines with Crow's Foot cardinality markers on each end.

use std::collections::HashMap;

use crate::parse::{ErDiagram, FlowDirection};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_points, SvgBuilder};
use super::geometry::{clip_rect_ends, fit_edges};
use super::theme::Theme;

mod entity;
mod relation;

use entity::{draw_entity, entity_size};
use relation::{draw_relation, CARD_MAX_REACH};

const CANVAS_PAD: f64 = 24.0;

pub(crate) fn render(d: &ErDiagram, theme: &Theme) -> String {
    if d.entities.is_empty() {
        return SvgBuilder::new(40.0, 40.0).theme(theme).finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d
        .entities
        .iter()
        .map(|e| entity_size(e, theme.font_size))
        .collect();
    let id_to_u32: HashMap<String, NodeId> = d
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.name.clone(), i as NodeId))
        .collect();
    let nodes: Vec<NodeId> = (0..d.entities.len() as NodeId).collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .relations
        .iter()
        .filter_map(|r| Some((*id_to_u32.get(&r.left)?, *id_to_u32.get(&r.right)?)))
        .collect();
    // Sugiyama only lays out top-down; for LR/RL swap node sizes so the
    // transposed layout reserves the right footprint (as flowchart/class do).
    let node_size_su: HashMap<NodeId, (f64, f64)> = d
        .entities
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
    // A relation label sits centered on its edge, between the cardinality
    // glyphs at each end (#398). The default layer gap alone doesn't reserve
    // room for both the glyph reach *and* the label width, so tightly packed
    // entities (e.g. `direction LR` chains) render the label overlapping the
    // crow's-foot glyphs. Widen the gap between layers to fit the widest
    // labeled relation's glyph clearance + label, never narrower than default.
    let max_label_w = d
        .relations
        .iter()
        .filter(|r| !r.label.is_empty())
        .map(|r| super::metrics::text_width(&r.label, 7.0, theme.font_size) + 8.0)
        .fold(0.0_f64, f64::max);
    let layout_cfg = LayoutConfig {
        layer_gap: (CARD_MAX_REACH * 2.0 + max_label_w).max(LayoutConfig::default().layer_gap),
        ..LayoutConfig::default()
    };
    let layout = layout_with(&g, &layout_cfg).unwrap_or_default();
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
    let drawn = layout
        .edge_points
        .iter()
        .filter(|(_, p)| p.len() >= 2)
        .map(|(&(u, v), p)| {
            let pts: Vec<(f64, f64)> = p.iter().map(|&q| transform(q)).collect();
            curve_basis_points(&clip_rect_ends(&pts, sizes[u as usize], sizes[v as usize]))
        });
    let ((dx, dy), (width, height)) = fit_edges(drawn.flatten(), (width, height), CANVAS_PAD);
    let transform = move |p: (f64, f64)| {
        let (x, y) = transform(p);
        (x + dx, y + dy)
    };

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    // Draw every relation's path first, collecting each labeled one's clipped
    // polyline, then resolve all label positions in one pass so two unrelated
    // relations whose labels would collide land apart.
    let mut label_slots: Vec<super::label_layout::LabelSlot> = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for rel in &d.relations {
        let (Some(&u), Some(&v)) = (id_to_u32.get(&rel.left), id_to_u32.get(&rel.right)) else {
            continue;
        };
        let Some(raw_pts) = layout.edge_points.get(&(u, v)) else {
            continue;
        };
        if raw_pts.len() < 2 {
            continue;
        }
        let pts: Vec<(f64, f64)> = raw_pts.iter().map(|&p| transform(p)).collect();
        let clipped = draw_relation(&mut svg, &pts, rel, &sizes, &id_to_u32, theme);
        if !rel.label.is_empty() && !clipped.is_empty() {
            let w = super::metrics::text_width(&rel.label, 7.0, theme.font_size) + 8.0;
            label_slots.push(super::label_layout::LabelSlot {
                polyline: clipped,
                size: (w, 18.0),
            });
            labels.push(&rel.label);
        }
    }
    let label_positions = super::label_layout::layout_edge_labels(&label_slots);
    for (pos, text) in label_positions.into_iter().zip(labels) {
        super::label::draw_edge_label(&mut svg, pos, text, theme);
    }

    for (i, e) in d.entities.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        draw_entity(&mut svg, center, sizes[i], e, &d.class_defs, theme);
    }

    svg.finish()
}

#[cfg(test)]
mod tests;
