//! Layer assignment (longest-path ranking) + dummy node insertion.
//!
//! After cycle removal the internal graph is a DAG. A topological sort lets us
//! propagate `rank[v] = max(rank[u] + 1)` over all predecessors. Edges that
//! skip layers are then split: a dummy node is inserted at each intermediate
//! rank and the original segment is replaced by a chain. All chain segments
//! share the same `original` index so edge routing can stitch them back.

use std::collections::HashSet;

use super::work::{InternalEdge, Work};

/// Rank, optionally pull sources down (`LayoutConfig::pull_sources`), then
/// move `first`/`last` (internal indices) onto their own outermost layers
/// before dummies are inserted, so edges to them get routed through every
/// intermediate layer.
pub(crate) fn assign_pinned(w: &mut Work, first: &[usize], last: &[usize], pull_sources: bool) {
    let first_set: HashSet<usize> = first.iter().copied().collect();
    let last_set: HashSet<usize> = last.iter().copied().collect();
    let topo = rank_longest_path(w);
    if !first.is_empty() {
        for r in w.rank.iter_mut() {
            *r += 1;
        }
        for &v in first {
            w.rank[v] = 0;
        }
    }
    if !last.is_empty() {
        let bottom = (0..w.node_count())
            .filter(|v| !last_set.contains(v))
            .map(|v| w.rank[v])
            .max()
            .map_or(0, |r| r + 1);
        for &v in last {
            w.rank[v] = bottom;
        }
    }
    // After pinning, so a source feeding only bottom ports sinks to the last
    // real layer instead of hovering a layer above its siblings.
    if pull_sources {
        let floor = usize::from(!first.is_empty());
        pull_sources_down(w, &topo, &first_set, &last_set, floor);
    }
    insert_dummies(w);
    rebuild_layers(w);
}

/// Longest-path ranking; returns the topological order it used.
fn rank_longest_path(w: &mut Work) -> Vec<usize> {
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
    topo
}

/// Move every unpinned source to the layer just above its nearest
/// successor, but never onto the first-pinned layer (`floor`). Sweeping in
/// reverse topological order sees every successor's final rank first; a
/// source's successors all rank above it, so the move never breaks an edge.
fn pull_sources_down(
    w: &mut Work,
    topo: &[usize],
    first: &HashSet<usize>,
    last: &HashSet<usize>,
    floor: usize,
) {
    for &u in topo.iter().rev() {
        let pinned = first.contains(&u) || last.contains(&u);
        if !w.in_e[u].is_empty() || w.out_e[u].is_empty() || pinned {
            continue;
        }
        let nearest = w.out_e[u]
            .iter()
            .map(|&e| w.rank[w.edges[e].dst])
            .min()
            .unwrap_or(1);
        w.rank[u] = nearest.saturating_sub(1).max(floor).max(w.rank[u]);
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
    use super::super::work::{NodeKind, Work};
    use super::super::{Graph, NodeId};
    use super::*;
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
        assign_pinned(&mut w, &[], &[], false);
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
        assign_pinned(&mut w, &[], &[], false);
        assert_eq!(w.rank[w.real_idx[&1]], 0);
        assert_eq!(w.rank[w.real_idx[&2]], 1);
        assert_eq!(w.rank[w.real_idx[&3]], 1);
        assert_eq!(w.rank[w.real_idx[&4]], 2);
    }

    #[test]
    fn pinned_nodes_take_the_outermost_layers() {
        // 1 -> 2 -> 3, plus port 9 -> 2 (pinned first) and 1 -> 8 (pinned last).
        let mut w = make(&[1, 2, 3, 8, 9], &[(1, 2), (2, 3), (9, 2), (1, 8)]);
        let first = [w.real_idx[&9]];
        let last = [w.real_idx[&8]];
        assign_pinned(&mut w, &first, &last, false);
        let rank = |id: NodeId| w.rank[w.real_idx[&id]];
        assert_eq!(rank(9), 0);
        assert!(rank(1) >= 1);
        assert_eq!(rank(8), rank(3) + 1);
        // The pinned layers hold nothing but their pinned nodes.
        assert_eq!(w.layers[0], vec![w.real_idx[&9]]);
        let last_layer = w.layers.last().unwrap();
        assert!(last_layer.contains(&w.real_idx[&8]));
        assert!(!last_layer.contains(&w.real_idx[&3]));
    }

    #[test]
    fn pulled_source_sits_just_above_its_target() {
        // 1 -> 2 -> 3 -> 4, and source 9 feeds only 4.
        let mut w = make(&[1, 2, 3, 4, 9], &[(1, 2), (2, 3), (3, 4), (9, 4)]);
        assign_pinned(&mut w, &[], &[], true);
        let rank = |id: NodeId| w.rank[w.real_idx[&id]];
        assert_eq!(rank(9), rank(4) - 1);
        // Without pulling, longest-path leaves it on the first layer.
        let mut w = make(&[1, 2, 3, 4, 9], &[(1, 2), (2, 3), (3, 4), (9, 4)]);
        assign_pinned(&mut w, &[], &[], false);
        assert_eq!(w.rank[w.real_idx[&9]], 0);
    }

    #[test]
    fn pulled_source_joins_the_last_layer_above_bottom_ports() {
        // 9 (first-pinned port) -> 1 -> 8 (last-pinned port); source 5 feeds
        // only port 7 (last-pinned). 5 belongs next to 1, not a layer above.
        let mut w = make(&[1, 5, 7, 8, 9], &[(9, 1), (1, 8), (5, 7)]);
        let first = [w.real_idx[&9]];
        let last = [w.real_idx[&8], w.real_idx[&7]];
        assign_pinned(&mut w, &first, &last, true);
        let rank = |id: NodeId| w.rank[w.real_idx[&id]];
        assert_eq!(rank(5), rank(1));
        assert!(rank(5) >= 1, "never on the first-pinned layer");
    }

    #[test]
    fn span_three_inserts_two_dummies() {
        // 1 -> 2 -> 3 -> 4 and a direct 1 -> 4 (span 3, needs 2 dummies)
        let mut w = make(&[1, 2, 3, 4], &[(1, 2), (2, 3), (3, 4), (1, 4)]);
        assign_pinned(&mut w, &[], &[], false);
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
