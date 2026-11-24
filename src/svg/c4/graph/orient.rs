//! Choosing which edges of a cyclic cluster graph to lay out reversed.
//!
//! sugiyama's own DFS cycle removal reverses whichever edge its traversal
//! happens to meet last, so the minority direction can win and drag the main
//! flow upwards. Here only edges inside a strongly connected component can
//! ever be reversed, and each small component is ordered *optimally* by a
//! dynamic programme over subsets. The cost counts every reversed relation
//! (parallel ones individually, so the majority direction wins) plus, at
//! `INVERSION_COST`, every node placed before one that is fed from nearer
//! the graph's sources: a chain such as Events → Detector → Workflow keeps
//! running forwards even when breaking it would save a single reversal.
//! Ties go to the node nearer the sources, then the earlier-declared one.
//! Components too big for the exact search fall back to the greedy
//! Eades–Lin–Smyth heuristic.

/// Largest component ordered exactly (2^n states).
const EXACT_MAX: usize = 12;
/// Cost units: a reversed relation costs `REVERSAL_COST`, a node placed
/// before a node fed from nearer the sources `INVERSION_COST` (1.5×).
const REVERSAL_COST: u32 = 2;
const INVERSION_COST: u32 = 3;

/// For each of `edges` (over nodes `0..n`, numbered in declaration order),
/// whether laying it out reversed makes the whole graph acyclic. Edges
/// between components and self-loops are never reversed.
pub(super) fn backward_edges(n: usize, edges: &[(usize, usize)]) -> Vec<bool> {
    let comp = components(n, edges);
    let depth = source_depth(n, edges, &comp);
    let mut pos = vec![0usize; n];
    let n_comp = comp.iter().copied().max().map_or(0, |m| m + 1);
    for c in 0..n_comp {
        let members: Vec<usize> = (0..n).filter(|&v| comp[v] == c).collect();
        let order = if members.len() <= 1 {
            members
        } else if members.len() <= EXACT_MAX {
            exact_order(&members, edges, &depth)
        } else {
            greedy_order(&members, edges)
        };
        for (i, v) in order.into_iter().enumerate() {
            pos[v] = i;
        }
    }
    edges
        .iter()
        .map(|&(u, v)| u != v && comp[u] == comp[v] && pos[u] > pos[v])
        .collect()
}

/// Strongly connected components (Tarjan, iterative): `comp[v]` is the
/// component of `v`; only membership matters, not the numbering.
fn components(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut adj = vec![Vec::new(); n];
    for &(u, v) in edges {
        adj[u].push(v);
    }
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut comp = vec![usize::MAX; n];
    let (mut next_index, mut next_comp) = (0, 0);
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        // (node, next child position)
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next_index;
        low[root] = next_index;
        next_index += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (v, ref mut child)) = call.last_mut() {
            if let Some(&w) = adj[v].get(*child) {
                *child += 1;
                if index[w] == usize::MAX {
                    index[w] = next_index;
                    low[w] = next_index;
                    next_index += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            call.pop();
            if let Some(&(parent, _)) = call.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    comp[w] = next_comp;
                    if w == v {
                        break;
                    }
                }
                next_comp += 1;
            }
        }
    }
    comp
}

/// Longest chain of edges *between* components leading into each node —
/// how far down from the graph's sources it is fed. A node inside a
/// component that nothing outside feeds is only reached around the cycle,
/// so it ranks after every node that is (`usize::MAX`).
fn source_depth(n: usize, edges: &[(usize, usize)], comp: &[usize]) -> Vec<usize> {
    let cross: Vec<(usize, usize)> = edges
        .iter()
        .copied()
        .filter(|&(u, v)| comp[u] != comp[v])
        .collect();
    let mut depth = vec![0usize; n];
    // The condensation is acyclic, so n relaxation rounds settle it.
    for _ in 0..n {
        let mut changed = false;
        for &(u, v) in &cross {
            if depth[v] < depth[u] + 1 {
                depth[v] = depth[u] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let fed: Vec<bool> = (0..n).map(|v| cross.iter().any(|&(_, b)| b == v)).collect();
    let in_cycle: Vec<bool> = (0..n)
        .map(|v| (0..n).any(|u| u != v && comp[u] == comp[v]))
        .collect();
    (0..n)
        .map(|v| {
            if in_cycle[v] && !fed[v] {
                usize::MAX
            } else {
                depth[v]
            }
        })
        .collect()
}

/// The cheapest order of `members` (see the module docs); ties go to the node
/// with the smaller `(depth, index)` — fed from nearer the sources, then
/// declared earlier.
fn exact_order(members: &[usize], edges: &[(usize, usize)], depth: &[usize]) -> Vec<usize> {
    let m = members.len();
    let local = |v: usize| members.iter().position(|&x| x == v);
    let mut w = vec![vec![0u32; m]; m];
    for &(u, v) in edges {
        if let (Some(a), Some(b)) = (local(u), local(v)) {
            if a != b {
                w[a][b] += 1;
            }
        }
    }
    // Appending `v` after the placed set reverses its edges into that set,
    // and inverts every placed node fed from further down than `v`.
    let d = |i: usize| depth[members[i]];
    let cost = |v: usize, placed: usize| -> u32 {
        (0..m)
            .filter(|&u| placed & (1 << u) != 0)
            .map(|u| REVERSAL_COST * w[v][u] + INVERSION_COST * u32::from(d(u) > d(v)))
            .sum()
    };
    let full = (1usize << m) - 1;
    // best[mask]: the cheapest way to order the nodes not yet in `mask`.
    let mut best = vec![0u32; full + 1];
    for mask in (0..full).rev() {
        best[mask] = (0..m)
            .filter(|&v| mask & (1 << v) == 0)
            .map(|v| cost(v, mask) + best[mask | (1 << v)])
            .min()
            .unwrap_or(0);
    }
    let mut order = Vec::with_capacity(m);
    let mut mask = 0usize;
    while mask != full {
        let next = (0..m)
            .filter(|&v| mask & (1 << v) == 0)
            .filter(|&v| cost(v, mask) + best[mask | (1 << v)] == best[mask])
            .min_by_key(|&v| (depth[members[v]], members[v]));
        let Some(v) = next else { break };
        order.push(members[v]);
        mask |= 1 << v;
    }
    order
}

/// Eades–Lin–Smyth on one component: sinks last, sources first, otherwise
/// the node whose outgoing edges most outnumber its incoming ones (ties to
/// the earlier-declared node). Degrees are kept up to date as nodes go.
fn greedy_order(members: &[usize], edges: &[(usize, usize)]) -> Vec<usize> {
    let m = members.len();
    let local: std::collections::HashMap<usize, usize> =
        members.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    let mut out_adj = vec![Vec::new(); m];
    let mut in_adj = vec![Vec::new(); m];
    for &(u, v) in edges {
        if let (Some(&a), Some(&b)) = (local.get(&u), local.get(&v)) {
            if a != b {
                out_adj[a].push(b);
                in_adj[b].push(a);
            }
        }
    }
    let mut out_deg: Vec<i64> = out_adj.iter().map(|a| a.len() as i64).collect();
    let mut in_deg: Vec<i64> = in_adj.iter().map(|a| a.len() as i64).collect();
    let mut alive = vec![true; m];
    let (mut head, mut tail) = (Vec::with_capacity(m), Vec::new());
    for _ in 0..m {
        let live = || (0..m).filter(|&v| alive[v]);
        let pick = if let Some(v) = live().find(|&v| out_deg[v] == 0) {
            tail.push(v);
            v
        } else if let Some(v) = live().find(|&v| in_deg[v] == 0) {
            head.push(v);
            v
        } else {
            let Some(v) = live().max_by_key(|&v| (out_deg[v] - in_deg[v], std::cmp::Reverse(v)))
            else {
                break;
            };
            head.push(v);
            v
        };
        alive[pick] = false;
        for &w in &out_adj[pick] {
            in_deg[w] -= 1;
        }
        for &w in &in_adj[pick] {
            out_deg[w] -= 1;
        }
    }
    head.extend(tail.into_iter().rev());
    head.into_iter().map(|i| members[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acyclic(n: usize, edges: &[(usize, usize)], back: &[bool]) -> bool {
        let oriented: Vec<(usize, usize)> = edges
            .iter()
            .zip(back)
            .filter(|((u, v), _)| u != v)
            .map(|(&(u, v), &b)| if b { (v, u) } else { (u, v) })
            .collect();
        let mut indeg = vec![0usize; n];
        for &(_, v) in &oriented {
            indeg[v] += 1;
        }
        let mut queue: Vec<usize> = (0..n).filter(|&u| indeg[u] == 0).collect();
        let mut seen = 0;
        while let Some(u) = queue.pop() {
            seen += 1;
            for &(a, b) in &oriented {
                if a == u {
                    indeg[b] -= 1;
                    if indeg[b] == 0 {
                        queue.push(b);
                    }
                }
            }
        }
        seen == n
    }

    #[test]
    fn majority_direction_wins_between_two_nodes() {
        let edges = [(0, 1), (1, 0), (1, 0), (1, 0)];
        assert_eq!(backward_edges(2, &edges), vec![true, false, false, false]);
        let edges = [(1, 0), (0, 1), (0, 1), (0, 1)];
        assert_eq!(backward_edges(2, &edges), vec![true, false, false, false]);
    }

    #[test]
    fn acyclic_graph_keeps_every_edge() {
        let edges = [(0, 1), (1, 2), (0, 2), (3, 2)];
        assert!(backward_edges(4, &edges).iter().all(|&b| !b));
    }

    #[test]
    fn result_is_acyclic_and_minimal() {
        let edges = [(0, 1), (1, 2), (2, 0), (2, 3), (3, 1), (1, 1)];
        let back = backward_edges(4, &edges);
        assert!(!back[5], "self-loop untouched");
        assert!(acyclic(4, &edges, &back));
        // Two cycles share 1 -> 2: reversing just that edge breaks both.
        assert_eq!(back.iter().filter(|&&b| b).count(), 1);
    }

    #[test]
    fn source_chain_runs_forward() {
        // The #396 root: feeds(0) -> detect(1) -> flow(2); flow -> core(3)
        // three times, core -> flow twice, core -> detect twice; monitor(4)
        // feeds flow, user(5) -> ws(6) -> core. Reversing just
        // detect -> flow would be one reversal cheaper, but the chain from
        // the source keeps running forward: detect, flow, core.
        let edges = [
            (0, 1),
            (1, 2),
            (2, 3),
            (2, 3),
            (2, 3),
            (3, 2),
            (3, 2),
            (3, 1),
            (3, 1),
            (4, 2),
            (5, 6),
            (6, 3),
        ];
        let back = backward_edges(7, &edges);
        assert!(acyclic(7, &edges, &back));
        assert!(!back[0] && !back[1], "feeds -> detect -> flow forward");
        assert!(!back[2] && !back[3] && !back[4], "flow -> core forward");
        assert!(back[5] && back[6] && back[7] && back[8]);
    }

    #[test]
    fn big_components_use_the_greedy_order() {
        // A 20-node ring plus chords: still acyclic afterwards.
        let n = 20;
        let mut edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
        edges.extend((0..n).step_by(3).map(|i| (i, (i + 7) % n)));
        let back = backward_edges(n, &edges);
        assert!(acyclic(n, &edges, &back));
    }
}
