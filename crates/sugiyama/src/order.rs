//! Crossing minimization via barycenter sweep.
//!
//! Each layer is repeatedly reordered by the average position of its
//! neighbors in the adjacent (fixed) layer. We alternate sweep direction
//! (down, then up) and keep the best ordering seen so far. Termination is
//! either max iterations or several iterations without improvement.

use crate::work::Work;

pub(crate) fn minimize_crossings(w: &mut Work, max_iter: usize) {
    if w.layers.len() <= 1 {
        return;
    }

    let mut best_crossings = count_crossings(w);
    let mut best_layers = w.layers.clone();
    let mut no_improve = 0;

    for iter in 0..max_iter {
        if iter % 2 == 0 {
            sweep_down(w);
        } else {
            sweep_up(w);
        }
        let c = count_crossings(w);
        if c < best_crossings {
            best_crossings = c;
            best_layers = w.layers.clone();
            no_improve = 0;
        } else {
            no_improve += 1;
            if no_improve >= 4 {
                break;
            }
        }
        if c == 0 {
            break;
        }
    }

    w.layers = best_layers;
    for layer in &w.layers {
        for (i, &v) in layer.iter().enumerate() {
            w.pos[v] = i;
        }
    }
}

fn sweep_down(w: &mut Work) {
    for l in 1..w.layers.len() {
        reorder_layer(w, l, l - 1);
    }
}

fn sweep_up(w: &mut Work) {
    for l in (0..w.layers.len().saturating_sub(1)).rev() {
        reorder_layer(w, l, l + 1);
    }
}

fn reorder_layer(w: &mut Work, layer_idx: usize, neighbor_layer_idx: usize) {
    let look_up = neighbor_layer_idx < layer_idx;
    let layer = w.layers[layer_idx].clone();

    let mut keyed: Vec<(f64, usize, usize)> = layer
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let bary = barycenter(w, v, look_up).unwrap_or(i as f64);
            (bary, i, v)
        })
        .collect();

    keyed.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });

    let new_layer: Vec<usize> = keyed.into_iter().map(|(_, _, v)| v).collect();
    for (i, &v) in new_layer.iter().enumerate() {
        w.pos[v] = i;
    }
    w.layers[layer_idx] = new_layer;
}

fn barycenter(w: &Work, v: usize, look_up: bool) -> Option<f64> {
    let edges = if look_up { &w.in_e[v] } else { &w.out_e[v] };
    if edges.is_empty() {
        return None;
    }
    let sum: usize = edges
        .iter()
        .map(|&e| {
            let other = if look_up {
                w.edges[e].src
            } else {
                w.edges[e].dst
            };
            w.pos[other]
        })
        .sum();
    Some(sum as f64 / edges.len() as f64)
}

fn count_crossings(w: &Work) -> usize {
    let mut total = 0usize;
    for l in 0..w.layers.len().saturating_sub(1) {
        let mut edges_l: Vec<(usize, usize)> = Vec::new();
        for &u in &w.layers[l] {
            for &e in &w.out_e[u] {
                let v = w.edges[e].dst;
                if w.rank[v] == l + 1 {
                    edges_l.push((w.pos[u], w.pos[v]));
                }
            }
        }
        for i in 0..edges_l.len() {
            for j in i + 1..edges_l.len() {
                let (a, b) = (edges_l[i], edges_l[j]);
                if (a.0 < b.0 && a.1 > b.1) || (a.0 > b.0 && a.1 < b.1) {
                    total += 1;
                }
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::Work;
    use crate::{layer, Graph, NodeId};
    use std::collections::HashMap;

    fn build(nodes: &[NodeId], edges: &[(NodeId, NodeId)]) -> Work {
        let node_size: HashMap<_, _> = nodes.iter().map(|&n| (n, (10.0, 10.0))).collect();
        let g = Graph {
            nodes: nodes.to_vec(),
            edges: edges.to_vec(),
            node_size,
        };
        let mut w = Work::from_input(&g).unwrap();
        crate::cycle::remove(&mut w);
        layer::assign(&mut w);
        w
    }

    #[test]
    fn crossing_reduced_on_butterfly() {
        // 1 → 4, 2 → 3 forces a crossing if [1,2] above [3,4]; swapping the
        // lower layer to [4,3] eliminates it. Start in a "bad" order and verify
        // the sweep finds the planar embedding.
        let mut w = build(&[1, 2, 3, 4], &[(1, 4), (2, 3)]);
        let before = count_crossings(&w);
        minimize_crossings(&mut w, 24);
        let after = count_crossings(&w);
        assert!(after <= before);
        assert_eq!(after, 0);
    }

    #[test]
    fn single_chain_zero_crossings() {
        let mut w = build(&[1, 2, 3], &[(1, 2), (2, 3)]);
        minimize_crossings(&mut w, 24);
        assert_eq!(count_crossings(&w), 0);
    }
}
