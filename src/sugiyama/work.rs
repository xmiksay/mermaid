use std::collections::HashMap;

use super::{Graph, LayoutError, NodeId};

pub(crate) enum NodeKind {
    Real(NodeId),
    Dummy,
}

#[derive(Clone, Copy)]
pub(crate) struct InternalEdge {
    pub src: usize,
    pub dst: usize,
    pub original: usize,
}

/// Shared workspace mutated through the pipeline. All per-node arrays are kept
/// in lock-step length so internal indices are valid across stages.
pub(crate) struct Work {
    pub kinds: Vec<NodeKind>,
    pub real_idx: HashMap<NodeId, usize>,

    pub edges: Vec<InternalEdge>,
    pub out_e: Vec<Vec<usize>>,
    pub in_e: Vec<Vec<usize>>,

    pub w: Vec<f64>,
    pub h: Vec<f64>,
    pub rank: Vec<usize>,
    pub layers: Vec<Vec<usize>>,
    pub pos: Vec<usize>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,

    pub original_edges: Vec<(NodeId, NodeId)>,
    pub reversed: Vec<bool>,
    pub self_loop: Vec<bool>,
}

impl Work {
    pub fn from_input(g: &Graph) -> Result<Self, LayoutError> {
        let mut real_idx = HashMap::with_capacity(g.nodes.len());
        let mut kinds = Vec::with_capacity(g.nodes.len());
        let mut ws = Vec::with_capacity(g.nodes.len());
        let mut hs = Vec::with_capacity(g.nodes.len());

        for &n in &g.nodes {
            if real_idx.contains_key(&n) {
                return Err(LayoutError::DuplicateNode(n));
            }
            let (nw, nh) = *g.node_size.get(&n).ok_or(LayoutError::MissingSize(n))?;
            real_idx.insert(n, kinds.len());
            kinds.push(NodeKind::Real(n));
            ws.push(nw);
            hs.push(nh);
        }

        let n_nodes = kinds.len();
        let mut edges = Vec::with_capacity(g.edges.len());
        let mut out_e = vec![Vec::new(); n_nodes];
        let mut in_e = vec![Vec::new(); n_nodes];
        let mut self_loop = Vec::with_capacity(g.edges.len());

        for (i, &(u, v)) in g.edges.iter().enumerate() {
            let ui = *real_idx.get(&u).ok_or(LayoutError::UnknownNode(u))?;
            let vi = *real_idx.get(&v).ok_or(LayoutError::UnknownNode(v))?;
            edges.push(InternalEdge {
                src: ui,
                dst: vi,
                original: i,
            });
            if ui == vi {
                self_loop.push(true);
            } else {
                let e_idx = edges.len() - 1;
                out_e[ui].push(e_idx);
                in_e[vi].push(e_idx);
                self_loop.push(false);
            }
        }

        Ok(Self {
            kinds,
            real_idx,
            edges,
            out_e,
            in_e,
            w: ws,
            h: hs,
            rank: vec![0; n_nodes],
            layers: Vec::new(),
            pos: vec![0; n_nodes],
            x: vec![0.0; n_nodes],
            y: vec![0.0; n_nodes],
            original_edges: g.edges.clone(),
            reversed: vec![false; g.edges.len()],
            self_loop,
        })
    }

    pub fn add_dummy(&mut self) -> usize {
        let idx = self.kinds.len();
        self.kinds.push(NodeKind::Dummy);
        self.out_e.push(Vec::new());
        self.in_e.push(Vec::new());
        self.w.push(0.0);
        self.h.push(0.0);
        self.rank.push(0);
        self.pos.push(0);
        self.x.push(0.0);
        self.y.push(0.0);
        idx
    }

    pub fn reverse_edge(&mut self, e_idx: usize) {
        let e = self.edges[e_idx];
        self.out_e[e.src].retain(|&i| i != e_idx);
        self.in_e[e.dst].retain(|&i| i != e_idx);
        self.edges[e_idx] = InternalEdge {
            src: e.dst,
            dst: e.src,
            original: e.original,
        };
        self.out_e[e.dst].push(e_idx);
        self.in_e[e.src].push(e_idx);
        self.reversed[e.original] = !self.reversed[e.original];
    }

    pub fn node_count(&self) -> usize {
        self.kinds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sugiyama::Graph;

    fn graph(nodes: &[NodeId], edges: &[(NodeId, NodeId)]) -> Graph {
        let node_size = nodes.iter().map(|&n| (n, (10.0, 10.0))).collect();
        Graph {
            nodes: nodes.to_vec(),
            edges: edges.to_vec(),
            node_size,
        }
    }

    #[test]
    fn from_input_indexes_reals_and_adjacency() {
        let w = Work::from_input(&graph(&[1, 2, 3], &[(1, 2), (2, 3)])).unwrap();
        assert_eq!(w.node_count(), 3);
        assert_eq!(w.real_idx[&1], 0);
        assert_eq!(w.real_idx[&3], 2);
        // (1->2) recorded as an out-edge of 1 and an in-edge of 2.
        assert_eq!(w.out_e[w.real_idx[&1]].len(), 1);
        assert_eq!(w.in_e[w.real_idx[&2]].len(), 1);
        assert!(w.self_loop.iter().all(|&s| !s));
    }

    #[test]
    fn self_loop_is_flagged_and_kept_out_of_adjacency() {
        let w = Work::from_input(&graph(&[1], &[(1, 1)])).unwrap();
        assert_eq!(w.self_loop, vec![true]);
        // A self-loop shapes no adjacency so it can't drive layering.
        assert!(w.out_e[0].is_empty());
        assert!(w.in_e[0].is_empty());
    }

    #[test]
    fn duplicate_node_is_rejected() {
        let g = graph(&[1, 1], &[]);
        assert_eq!(
            Work::from_input(&g).err(),
            Some(LayoutError::DuplicateNode(1))
        );
    }

    #[test]
    fn unknown_edge_endpoint_is_rejected() {
        let g = graph(&[1], &[(1, 2)]);
        assert_eq!(
            Work::from_input(&g).err(),
            Some(LayoutError::UnknownNode(2))
        );
    }

    #[test]
    fn missing_size_is_rejected() {
        let g = Graph {
            nodes: vec![1, 2],
            edges: vec![],
            node_size: HashMap::from([(1, (10.0, 10.0))]),
        };
        assert_eq!(
            Work::from_input(&g).err(),
            Some(LayoutError::MissingSize(2))
        );
    }

    #[test]
    fn add_dummy_grows_all_arrays_in_lockstep() {
        let mut w = Work::from_input(&graph(&[1, 2], &[(1, 2)])).unwrap();
        let before = w.node_count();
        let idx = w.add_dummy();
        assert_eq!(idx, before);
        assert_eq!(w.node_count(), before + 1);
        for len in [w.w.len(), w.h.len(), w.rank.len(), w.x.len(), w.y.len()] {
            assert_eq!(len, before + 1);
        }
        assert!(matches!(w.kinds[idx], NodeKind::Dummy));
    }

    #[test]
    fn reverse_edge_swaps_endpoints_and_toggles_flag() {
        let mut w = Work::from_input(&graph(&[1, 2], &[(1, 2)])).unwrap();
        assert!(!w.reversed[0]);
        w.reverse_edge(0);
        let e = w.edges[0];
        assert_eq!(e.src, w.real_idx[&2]);
        assert_eq!(e.dst, w.real_idx[&1]);
        assert!(w.reversed[0]);
        // Adjacency followed the reversal.
        assert!(w.out_e[w.real_idx[&2]].contains(&0));
        assert!(w.in_e[w.real_idx[&1]].contains(&0));
        // Reversing again restores the original orientation.
        w.reverse_edge(0);
        assert!(!w.reversed[0]);
        assert_eq!(w.edges[0].src, w.real_idx[&1]);
    }
}
