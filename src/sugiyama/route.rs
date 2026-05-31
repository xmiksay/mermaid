//! Edge routing: stitch dummy positions back into a waypoint polyline per
//! original edge, applying any reversal recorded during cycle removal.

use std::collections::HashMap;

use super::work::{NodeKind, Work};
use super::{Layout, NodeId};

pub(crate) fn build(w: &Work) -> Layout {
    let mut node_pos: HashMap<NodeId, (f64, f64)> = HashMap::new();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (i, k) in w.kinds.iter().enumerate() {
        if let NodeKind::Real(id) = k {
            node_pos.insert(*id, (w.x[i], w.y[i]));
            let (hw, hh) = (w.w[i] / 2.0, w.h[i] / 2.0);
            min_x = min_x.min(w.x[i] - hw);
            max_x = max_x.max(w.x[i] + hw);
            min_y = min_y.min(w.y[i] - hh);
            max_y = max_y.max(w.y[i] + hh);
        }
    }

    // Group internal edges by their original-edge index.
    let mut by_original: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for e in &w.edges {
        if w.self_loop[e.original] {
            continue;
        }
        by_original
            .entry(e.original)
            .or_default()
            .push((e.src, e.dst));
    }

    let mut edge_points: HashMap<(NodeId, NodeId), Vec<(f64, f64)>> = HashMap::new();
    for (orig_idx, &(orig_u, orig_v)) in w.original_edges.iter().enumerate() {
        if w.self_loop[orig_idx] {
            if let Some(&(cx, cy)) = node_pos.get(&orig_u) {
                let real_i = w.real_idx[&orig_u];
                let h = w.h[real_i] / 2.0;
                let waypoints = vec![
                    (cx, cy - h),
                    (cx + 40.0, cy - h - 40.0),
                    (cx + 40.0, cy - h - 10.0),
                    (cx, cy - h),
                ];
                edge_points.insert((orig_u, orig_v), waypoints);
            }
            continue;
        }

        let Some(segments) = by_original.get(&orig_idx) else {
            continue;
        };
        if segments.is_empty() {
            continue;
        }

        let mut sorted = segments.clone();
        sorted.sort_by_key(|&(s, _)| w.rank[s]);

        let mut chain: Vec<usize> = Vec::with_capacity(sorted.len() + 1);
        chain.push(sorted[0].0);
        for (_, d) in &sorted {
            chain.push(*d);
        }

        let mut waypoints: Vec<(f64, f64)> = chain.iter().map(|&n| (w.x[n], w.y[n])).collect();
        if w.reversed[orig_idx] {
            waypoints.reverse();
        }
        edge_points.insert((orig_u, orig_v), waypoints);
    }

    let width = if min_x.is_finite() { max_x - min_x } else { 0.0 };
    let height = if min_y.is_finite() { max_y - min_y } else { 0.0 };

    Layout {
        node_pos,
        edge_points,
        width,
        height,
    }
}
