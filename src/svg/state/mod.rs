//! State diagram renderer. Layout via sugiyama, custom shapes for
//! start/end/choice/fork/join pseudo-states.

use std::collections::{HashMap, HashSet};

use crate::parse::{FlowDirection, StateDiagram};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::SvgBuilder;
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
/// Arc-length by which an opposite-pair transition's label is nudged toward its
/// own source, so the two labels of the pair clear each other's opaque
/// background (#312).
const LABEL_STAGGER: f64 = 15.0;

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

    // Canvas extent from node boundaries and cluster frames. A frame reserves
    // header room above its members for the title, so its top/left can fall
    // above/left of the topmost node — measure both corners of every box and
    // shift everything back into a positive CANVAS_PAD margin so the title band
    // is not clipped by the viewBox top edge (issue #242).
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
    for &(bx0, by0, bx1, by1) in boxes.values() {
        min_x = min_x.min(bx0);
        min_y = min_y.min(by0);
        max_x = max_x.max(bx1);
        max_y = max_y.max(by1);
    }
    let shift_x = (CANVAS_PAD - min_x).max(0.0);
    let shift_y = (CANVAS_PAD - min_y).max(0.0);
    if shift_x != 0.0 || shift_y != 0.0 {
        for p in pos.values_mut() {
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
    let width = max_x + shift_x + CANVAS_PAD;
    let height = max_y + shift_y + CANVAS_PAD;

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    define_marker(&mut svg, theme);

    // Cluster frames first (under nodes/edges) so labels stay legible.
    draw_composites(&mut svg, d, &boxes, &dividers, theme);

    // Opposite-pair transitions (`A --> B` alongside `B --> A`) bow onto
    // separate curves, but their labels still land at the shared midpoint y and
    // one's opaque background occludes the other (#312). Detect the pairs so the
    // label of each is nudged back along its own arc toward its source.
    let reversed_pairs: HashSet<(&str, &str)> = d
        .transitions
        .iter()
        .filter(|t| {
            d.transitions
                .iter()
                .any(|o| o.from == t.to && o.to == t.from)
        })
        .map(|t| (t.from.as_str(), t.to.as_str()))
        .collect();

    for tr in &d.transitions {
        let (Some(start), Some(end)) = (
            endpoint_clip(&tr.from, &id_to_u32, &d.states, &sizes, &pos, &boxes),
            endpoint_clip(&tr.to, &id_to_u32, &d.states, &sizes, &pos, &boxes),
        ) else {
            continue;
        };
        // Real node→node transitions keep their routed polyline; an endpoint
        // that is a composite cluster has no layout route, so draw a straight
        // connector clipped to the cluster box.
        let pts: Vec<(f64, f64)> = match (id_to_u32.get(&tr.from), id_to_u32.get(&tr.to)) {
            (Some(&u), Some(&v)) => match layout.edge_points.get(&(u, v)) {
                // Both endpoints share a region, hence the same stacking offset;
                // shift the whole routed polyline so it tracks the moved nodes.
                Some(p) if p.len() >= 2 => {
                    let (ox, oy) = node_offset.get(&u).copied().unwrap_or((0.0, 0.0));
                    p.iter()
                        .map(|&q| {
                            let (x, y) = transform(q);
                            (x + ox + shift_x, y + oy + shift_y)
                        })
                        .collect()
                }
                _ => vec![start.center, end.center],
            },
            _ => vec![start.center, end.center],
        };
        // For an opposite pair, pull the label back toward this edge's own
        // source; the two edges run in opposite directions, so their labels
        // separate along the shared axis and stop overlapping.
        let label_offset = if reversed_pairs.contains(&(tr.from.as_str(), tr.to.as_str())) {
            -LABEL_STAGGER
        } else {
            0.0
        };
        draw_transition(&mut svg, &pts, tr, &start, &end, label_offset, theme);
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
