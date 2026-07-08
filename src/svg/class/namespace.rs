//! Namespace frames for class diagrams: compute each frame strictly from its
//! member nodes, then push any non-member that would straddle the border clear
//! of it. The shared sugiyama layout is cluster-agnostic, so an outside class
//! can land horizontally inside a namespace's member bounding box; this pass
//! restores clean containment after layout.

use std::collections::{HashMap, HashSet};

use crate::parse::ClassDiagram;
use crate::sugiyama::NodeId;

/// Horizontal clearance kept between a namespace frame and a non-member node
/// that had to be pushed out of it.
const CLEAR_GAP: f64 = 24.0;

/// A namespace's drawn frame in screen space, tied back to `d.namespaces[idx]`.
pub(super) struct NsFrame {
    pub idx: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl NsFrame {
    fn x1(&self) -> f64 {
        self.x + self.w
    }
    fn y1(&self) -> f64 {
        self.y + self.h
    }
    fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
}

/// Compute each namespace's frame rect from (and only from) its member nodes.
/// A shallower (smaller-`depth`) frame gets more padding so an outer namespace
/// visibly wraps a nested one.
pub(super) fn frames(
    d: &ClassDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &HashMap<NodeId, (f64, f64)>,
    sizes: &[(f64, f64)],
) -> Vec<NsFrame> {
    let max_depth = d.namespaces.iter().map(|n| n.depth).max().unwrap_or(0);
    let mut out = Vec::new();
    for (idx, ns) in d.namespaces.iter().enumerate() {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for name in &ns.class_names {
            let Some(&u) = id_to_u32.get(name) else {
                continue;
            };
            let (cx, cy) = pos[&u];
            let (w, h) = sizes[u as usize];
            min_x = min_x.min(cx - w / 2.0);
            max_x = max_x.max(cx + w / 2.0);
            min_y = min_y.min(cy - h / 2.0);
            max_y = max_y.max(cy + h / 2.0);
        }
        if !min_x.is_finite() {
            continue;
        }
        let pad = 12.0 + (max_depth - ns.depth) as f64 * 10.0;
        let header_h = 18.0;
        out.push(NsFrame {
            idx,
            x: min_x - pad,
            y: min_y - pad - header_h,
            w: (max_x - min_x) + pad * 2.0,
            h: (max_y - min_y) + pad * 2.0 + header_h,
        });
    }
    out
}

/// Push every class that belongs to no namespace but overlaps a namespace frame
/// horizontally clear of it. Members never move, so the precomputed frames stay
/// valid; each moved node's incident edges are re-straightened to its new center
/// (it now sits in open margin space, so a direct line can't collide).
pub(super) fn separate_outsiders(
    d: &ClassDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
    frames: &[NsFrame],
    pos: &mut HashMap<NodeId, (f64, f64)>,
    edge_pts: &mut HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) {
    let namespaced: HashSet<NodeId> = d
        .namespaces
        .iter()
        .flat_map(|ns| ns.class_names.iter())
        .filter_map(|n| id_to_u32.get(n).copied())
        .collect();

    let mut moved: HashSet<NodeId> = HashSet::new();
    for frame in frames {
        for (i, &(w, h)) in sizes.iter().enumerate() {
            let u = i as NodeId;
            if namespaced.contains(&u) {
                continue;
            }
            let (cx, cy) = pos[&u];
            let (nx0, nx1) = (cx - w / 2.0, cx + w / 2.0);
            let (ny0, ny1) = (cy - h / 2.0, cy + h / 2.0);
            let overlaps = nx0 < frame.x1() && nx1 > frame.x && ny0 < frame.y1() && ny1 > frame.y;
            if !overlaps {
                continue;
            }
            // Push out the nearer side to minimize how far the canvas grows.
            let dx = if cx >= frame.cx() {
                frame.x1() + CLEAR_GAP - nx0
            } else {
                frame.x - CLEAR_GAP - nx1
            };
            if let Some(p) = pos.get_mut(&u) {
                p.0 += dx;
                moved.insert(u);
            }
        }
    }

    if moved.is_empty() {
        return;
    }
    for rel in &d.relations {
        let (Some(&a), Some(&b)) = (id_to_u32.get(&rel.from), id_to_u32.get(&rel.to)) else {
            continue;
        };
        if moved.contains(&a) || moved.contains(&b) {
            edge_pts.insert((a, b), vec![pos[&a], pos[&b]]);
        }
    }
}
