//! ER diagram renderer. Entities are drawn as tables (name header + attribute
//! rows), connected by lines with Crow's Foot cardinality markers on each end.

use std::collections::HashMap;

use crate::parse::{ErDiagram, FlowDirection};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::SvgBuilder;
use super::theme::Theme;

mod entity;
mod relation;

use entity::{draw_entity, entity_size};
use relation::draw_relation;

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

    let mut svg = SvgBuilder::new(width, height).theme(theme);

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
        draw_relation(&mut svg, &pts, rel, &sizes, &id_to_u32, theme);
    }

    for (i, e) in d.entities.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        draw_entity(&mut svg, center, sizes[i], e, &d.class_defs, theme);
    }

    svg.finish()
}

#[cfg(test)]
mod tests;
