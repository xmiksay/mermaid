//! Layer assignment (longest-path ranking) + dummy node insertion.
//!
//! After cycle removal the internal graph is a DAG. A topological sort lets us
//! propagate `rank[v] = max(rank[u] + 1)` over all predecessors. Edges that
//! skip layers are then split: a dummy node is inserted at each intermediate
//! rank and the original segment is replaced by a chain. All chain segments
//! share the same `original` index so edge routing can stitch them back.

use crate::work::{InternalEdge, Work};

pub(crate) fn assign(w: &mut Work) {
    rank_longest_path(w);
    insert_dummies(w);
    rebuild_layers(w);
}

fn rank_longest_path(w: &mut Work) {
    let n = w.node_count();
    w.rank = vec![0; n];

    let mut indeg: Vec<usize> = (0..n).map(|v| w.in_e[v].len()).collect();
    let mut queue: Vec<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
    let mut head = 0;
    let mut topo: Vec<usize> = Vec::with_capacity(n);
    while head < queue.len() {
        let u = queue[head];
        head += 1;
        topo.push(u);
        for &e_idx in &w.out_e[u].clone() {
            let v = w.edges[e_idx].dst;
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push(v);
            }
        }
    }

    for &u in &topo {
        let r = w.rank[u];
        for &e_idx in &w.out_e[u].clone() {
            let v = w.edges[e_idx].dst;
            if w.rank[v] < r + 1 {
                w.rank[v] = r + 1;
            }
        }
    }
}

fn insert_dummies(w: &mut Work) {
    let edge_count_snapshot = w.edges.len();
    for e_idx in 0..edge_count_snapshot {
        let e = w.edges[e_idx];
        if w.self_loop[e.original] {
            continue;
        }
        let src = e.src;
        let dst = e.dst;
        let span = w.rank[dst].saturating_sub(w.rank[src]);
        if span <= 1 {
            continue;
        }

        // Chain: src -> d_1 -> d_2 -> ... -> d_{span-1} -> dst.
        // Reuse e_idx as the (src -> d_1) segment; append new edges for the rest.
        let mut prev = src;
        for k in 1..span {
            let d = w.add_dummy();
            w.rank[d] = w.rank[src] + k;

            if k == 1 {
                // Rewire e_idx to src -> d.
                w.in_e[dst].retain(|&i| i != e_idx);
                w.edges[e_idx].dst = d;
                w.in_e[d].push(e_idx);
            } else {
                let new_idx = w.edges.len();
                w.edges.push(InternalEdge {
                    src: prev,
                    dst: d,
                    original: e.original,
                });
                w.out_e[prev].push(new_idx);
                w.in_e[d].push(new_idx);
            }
            prev = d;
        }

        // Final segment prev -> dst.
        let new_idx = w.edges.len();
        w.edges.push(InternalEdge {
            src: prev,
            dst,
            original: e.original,
        });
        w.out_e[prev].push(new_idx);
        w.in_e[dst].push(new_idx);
    }
}

fn rebuild_layers(w: &mut Work) {
    let n = w.node_count();
    let max_rank = w.rank.iter().copied().max().unwrap_or(0);
    w.layers = vec![Vec::new(); max_rank + 1];
    for v in 0..n {
        w.layers[w.rank[v]].push(v);
    }
    w.pos = vec![0; n];
    for layer in &w.layers {
        for (i, &v) in layer.iter().enumerate() {
            w.pos[v] = i;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::{NodeKind, Work};
    use crate::{Graph, NodeId};
    use std::collections::HashMap;

    fn make(nodes: &[NodeId], edges: &[(NodeId, NodeId)]) -> Work {
        let node_size: HashMap<_, _> = nodes.iter().map(|&n| (n, (10.0, 10.0))).collect();
        let g = Graph {
            nodes: nodes.to_vec(),
            edges: edges.to_vec(),
            node_size,
        };
        Work::from_input(&g).unwrap()
    }

    #[test]
    fn linear_chain_ranks() {
        let mut w = make(&[1, 2, 3, 4], &[(1, 2), (2, 3), (3, 4)]);
        assign(&mut w);
        assert_eq!(w.rank[w.real_idx[&1]], 0);
        assert_eq!(w.rank[w.real_idx[&2]], 1);
        assert_eq!(w.rank[w.real_idx[&3]], 2);
        assert_eq!(w.rank[w.real_idx[&4]], 3);
        assert_eq!(w.layers.len(), 4);
    }

    #[test]
    fn fork_join_ranks() {
        // 1 -> 2 -> 4, 1 -> 3 -> 4
        let mut w = make(&[1, 2, 3, 4], &[(1, 2), (1, 3), (2, 4), (3, 4)]);
        assign(&mut w);
        assert_eq!(w.rank[w.real_idx[&1]], 0);
        assert_eq!(w.rank[w.real_idx[&2]], 1);
        assert_eq!(w.rank[w.real_idx[&3]], 1);
        assert_eq!(w.rank[w.real_idx[&4]], 2);
    }

    #[test]
    fn span_three_inserts_two_dummies() {
        // 1 -> 2 -> 3 -> 4 and a direct 1 -> 4 (span 3, needs 2 dummies)
        let mut w = make(&[1, 2, 3, 4], &[(1, 2), (2, 3), (3, 4), (1, 4)]);
        assign(&mut w);
        let n_dummies = w
            .kinds
            .iter()
            .filter(|k| matches!(k, NodeKind::Dummy))
            .count();
        assert_eq!(n_dummies, 2);
        // each layer must contain at least one node
        for layer in &w.layers {
            assert!(!layer.is_empty());
        }
        // every internal edge spans exactly one layer
        for e in &w.edges {
            if w.self_loop[e.original] {
                continue;
            }
            assert_eq!(w.rank[e.dst] - w.rank[e.src], 1);
        }
    }
}
