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
