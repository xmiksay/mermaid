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

    let width = if min_x.is_finite() {
        max_x - min_x
    } else {
        0.0
    };
    let height = if min_y.is_finite() {
        max_y - min_y
    } else {
        0.0
    };

    Layout {
        node_pos,
        edge_points,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sugiyama::{cycle, layer, order, Graph, LayoutConfig};

    fn build_layout(nodes: &[NodeId], edges: &[(NodeId, NodeId)]) -> Layout {
        let node_size = nodes.iter().map(|&n| (n, (40.0, 20.0))).collect();
        let g = Graph {
            nodes: nodes.to_vec(),
            edges: edges.to_vec(),
            node_size,
        };
        let mut w = Work::from_input(&g).unwrap();
        cycle::remove(&mut w);
        layer::assign(&mut w);
        order::minimize_crossings(&mut w, 24);
        super::super::coord::assign(&mut w, &LayoutConfig::default());
        build(&w)
    }

    // The invariant every downstream renderer clips against: a routed edge
    // always carries at least its two endpoints.
    #[test]
    fn every_edge_polyline_has_at_least_two_points() {
        let l = build_layout(&[1, 2, 3, 4, 5], &[(1, 2), (2, 3), (3, 4), (4, 5), (1, 5)]);
        assert_eq!(l.edge_points.len(), 5);
        for (edge, pts) in &l.edge_points {
            assert!(pts.len() >= 2, "edge {edge:?} routed with < 2 points");
        }
    }

    #[test]
    fn endpoints_sit_at_node_centers() {
        let l = build_layout(&[1, 2], &[(1, 2)]);
        let pts = &l.edge_points[&(1, 2)];
        assert_eq!(*pts.first().unwrap(), l.node_pos[&1]);
        assert_eq!(*pts.last().unwrap(), l.node_pos[&2]);
    }

    #[test]
    fn reversed_edge_keeps_user_direction() {
        // 3 -> 1 is a back-edge; the polyline still runs 3's center -> 1's center.
        let l = build_layout(&[1, 2, 3], &[(1, 2), (2, 3), (3, 1)]);
        let pts = &l.edge_points[&(3, 1)];
        assert!(pts.len() >= 2);
        assert_eq!(*pts.first().unwrap(), l.node_pos[&3]);
        assert_eq!(*pts.last().unwrap(), l.node_pos[&1]);
    }

    #[test]
    fn self_loop_routes_a_closed_four_point_loop() {
        let l = build_layout(&[1, 2], &[(1, 1), (1, 2)]);
        let loop_pts = l.edge_points.get(&(1, 1)).expect("self-loop routed");
        assert_eq!(loop_pts.len(), 4);
        // A self-loop starts and ends on the same node boundary point.
        assert_eq!(loop_pts.first(), loop_pts.last());
        assert!(loop_pts.len() >= 2);
    }

    #[test]
    fn isolated_node_has_no_edges_but_is_positioned() {
        let l = build_layout(&[1], &[]);
        assert!(l.node_pos.contains_key(&1));
        assert!(l.edge_points.is_empty());
    }
}
