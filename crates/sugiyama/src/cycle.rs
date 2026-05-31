//! Cycle removal via DFS back-edge reversal.
//!
//! Iterative DFS marks each vertex white -> gray -> black. An edge to a gray
//! vertex is a back edge in the DFS tree; we reverse it so layer assignment
//! sees a DAG. Reversal is recorded so edge routing can flip waypoints back.

use crate::work::Work;

pub(crate) fn remove(w: &mut Work) {
    let n = w.node_count();
    let mut state = vec![0u8; n]; // 0=white, 1=gray, 2=black
    let mut back_edges: Vec<usize> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        state[start] = 1;
        stack.push((start, 0));
        while let Some(&(u, i)) = stack.last() {
            if i < w.out_e[u].len() {
                let e_idx = w.out_e[u][i];
                stack.last_mut().unwrap().1 += 1;
                let v = w.edges[e_idx].dst;
                match state[v] {
                    0 => {
                        state[v] = 1;
                        stack.push((v, 0));
                    }
                    1 => back_edges.push(e_idx),
                    _ => {}
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }

    for e_idx in back_edges {
        w.reverse_edge(e_idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::Work;
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
    fn dag_unchanged() {
        let mut w = make(&[1, 2, 3], &[(1, 2), (2, 3), (1, 3)]);
        remove(&mut w);
        assert!(w.reversed.iter().all(|&r| !r));
    }

    #[test]
    fn simple_cycle_breaks() {
        let mut w = make(&[1, 2, 3], &[(1, 2), (2, 3), (3, 1)]);
        remove(&mut w);
        // Exactly one edge reversed to break the 3-cycle.
        let n_rev = w.reversed.iter().filter(|&&r| r).count();
        assert_eq!(n_rev, 1);
    }

    #[test]
    fn two_cycles() {
        // Two disjoint cycles
        let mut w = make(
            &[1, 2, 3, 4, 5, 6],
            &[(1, 2), (2, 1), (3, 4), (4, 5), (5, 3), (5, 6)],
        );
        remove(&mut w);
        // After reversal, the internal graph must be acyclic.
        let n = w.node_count();
        let mut indeg: Vec<usize> = (0..n).map(|v| w.in_e[v].len()).collect();
        let mut queue: Vec<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
        let mut head = 0;
        let mut visited = 0;
        while head < queue.len() {
            let u = queue[head];
            head += 1;
            visited += 1;
            for &e_idx in &w.out_e[u].clone() {
                let v = w.edges[e_idx].dst;
                indeg[v] -= 1;
                if indeg[v] == 0 {
                    queue.push(v);
                }
            }
        }
        assert_eq!(visited, n, "internal graph still has a cycle");
    }
}
