//! Y- and X-coordinate assignment.
//!
//! Y is straightforward: each layer occupies a horizontal band whose height
//! is the max node height in that layer; nodes are centered vertically.
//!
//! X is the hard part. Brandes & Köpf (2001) describe a four-pass algorithm
//! that averages {left,right} × {up,down} block alignments. We currently use
//! a simpler iterative median-relaxation with hard non-overlap constraints
//! between same-layer neighbors — fewer lines, valid layouts, but not as
//! tightly balanced as full Brandes-Köpf. The public API does not change if
//! we swap implementations later.

use super::work::Work;
use super::LayoutConfig;

pub(crate) fn assign(w: &mut Work, cfg: &LayoutConfig) {
    assign_y(w, cfg.layer_gap);
    assign_x(w, cfg.node_gap, cfg.max_coord_iter);
}

fn assign_y(w: &mut Work, layer_gap: f64) {
    let n = w.node_count();
    w.y = vec![0.0; n];
    let mut y_cursor = 0.0;
    for layer in &w.layers {
        let max_h = layer.iter().map(|&v| w.h[v]).fold(0.0_f64, f64::max);
        let band_h = if max_h > 0.0 { max_h } else { 20.0 };
        for &v in layer {
            w.y[v] = y_cursor + band_h / 2.0;
        }
        y_cursor += band_h + layer_gap;
    }
}

fn assign_x(w: &mut Work, node_gap: f64, max_iter: usize) {
    let n = w.node_count();

    // Phase 1: pack each layer left-to-right at minimum gap, centering each node.
    let mut x = vec![0.0_f64; n];
    for layer in &w.layers {
        let mut cursor = 0.0;
        for &v in layer {
            x[v] = cursor + w.w[v] / 2.0;
            cursor += w.w[v] + node_gap;
        }
    }

    // Phase 2: iteratively pull each node toward the median of its adjacent-layer
    // neighbors, clamped to the same-layer non-overlap interval. Sweep down then up.
    for _ in 0..max_iter {
        let mut max_shift: f64 = 0.0;
        for direction in 0..2 {
            let look_up = direction == 0;
            let layer_order: Vec<usize> = if look_up {
                (1..w.layers.len()).collect()
            } else if w.layers.is_empty() {
                vec![]
            } else {
                (0..w.layers.len() - 1).rev().collect()
            };
            for li in layer_order {
                let layer = w.layers[li].clone();
                for (i, &v) in layer.iter().enumerate() {
                    let edges = if look_up { &w.in_e[v] } else { &w.out_e[v] };
                    if edges.is_empty() {
                        continue;
                    }
                    let mut positions: Vec<f64> = edges
                        .iter()
                        .map(|&e| {
                            let u = if look_up {
                                w.edges[e].src
                            } else {
                                w.edges[e].dst
                            };
                            x[u]
                        })
                        .collect();
                    positions.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let m = positions.len();
                    let target = if m % 2 == 1 {
                        positions[m / 2]
                    } else {
                        (positions[m / 2 - 1] + positions[m / 2]) / 2.0
                    };

                    let hw = w.w[v] / 2.0;
                    let left_bound = if i > 0 {
                        let u = layer[i - 1];
                        x[u] + w.w[u] / 2.0 + node_gap + hw
                    } else {
                        f64::NEG_INFINITY
                    };
                    let right_bound = if i + 1 < layer.len() {
                        let u = layer[i + 1];
                        x[u] - w.w[u] / 2.0 - node_gap - hw
                    } else {
                        f64::INFINITY
                    };
                    let clamped = target.max(left_bound).min(right_bound);
                    let delta = (clamped - x[v]).abs();
                    if delta > max_shift {
                        max_shift = delta;
                    }
                    x[v] = clamped;
                }
            }
        }
        if max_shift < 0.5 {
            break;
        }
    }

    // Normalize: leftmost edge at x = 0.
    let mut min_x = f64::INFINITY;
    for v in 0..n {
        let left_edge = x[v] - w.w[v] / 2.0;
        if left_edge < min_x {
            min_x = left_edge;
        }
    }
    if min_x.is_finite() {
        for xv in x.iter_mut() {
            *xv -= min_x;
        }
    }
    w.x = x;
}

#[cfg(test)]
mod tests {
    use super::super::work::Work;
    use super::super::{cycle, layer, order, Graph, LayoutConfig, NodeId};
    use super::*;
    use std::collections::HashMap;

    fn build(nodes: &[NodeId], edges: &[(NodeId, NodeId)], size: (f64, f64)) -> Work {
        let node_size: HashMap<_, _> = nodes.iter().map(|&n| (n, size)).collect();
        let g = Graph {
            nodes: nodes.to_vec(),
            edges: edges.to_vec(),
            node_size,
        };
        let mut w = Work::from_input(&g).unwrap();
        cycle::remove(&mut w);
        layer::assign(&mut w);
        order::minimize_crossings(&mut w, 24);
        w
    }

    #[test]
    fn no_overlap_within_layer() {
        let mut w = build(
            &[1, 2, 3, 4, 5],
            &[(1, 2), (1, 3), (1, 4), (1, 5)],
            (40.0, 20.0),
        );
        assign(&mut w, &LayoutConfig::default());
        // All children share layer 1: verify they don't overlap horizontally.
        for layer in &w.layers {
            let mut by_x: Vec<(f64, usize)> = layer.iter().map(|&v| (w.x[v], v)).collect();
            by_x.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            for win in by_x.windows(2) {
                let (xa, va) = win[0];
                let (xb, vb) = win[1];
                let right_a = xa + w.w[va] / 2.0;
                let left_b = xb - w.w[vb] / 2.0;
                assert!(left_b >= right_a - 1e-6, "overlap in layer");
            }
        }
    }

    #[test]
    fn layers_increase_in_y() {
        let mut w = build(&[1, 2, 3], &[(1, 2), (2, 3)], (40.0, 20.0));
        assign(&mut w, &LayoutConfig::default());
        let y1 = w.y[w.real_idx[&1]];
        let y2 = w.y[w.real_idx[&2]];
        let y3 = w.y[w.real_idx[&3]];
        assert!(y1 < y2 && y2 < y3);
    }
}
