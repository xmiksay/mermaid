//! Sugiyama-style layered graph layout.
//!
//! Input: directed graph (cycles allowed) plus node sizes.
//! Output: (x, y) per node and waypoint polyline per edge.
//!
//! Pipeline:
//! 1. cycle removal (DFS back-edge reversal)
//! 2. layer assignment (longest path) + dummy node insertion
//! 3. crossing minimization (barycenter sweep)
//! 4. horizontal coordinate assignment
//! 5. edge routing (collect dummy positions as waypoints)

use std::collections::{HashMap, HashSet};
use thiserror::Error;

pub type NodeId = u32;

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: Vec<NodeId>,
    pub edges: Vec<(NodeId, NodeId)>,
    pub node_size: HashMap<NodeId, (f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub node_pos: HashMap<NodeId, (f64, f64)>,
    /// Polyline per original edge, including endpoints (in user-given direction).
    pub edge_points: HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
    /// Polyline per input edge *index* (same order as `Graph::edges`), without
    /// the opposite-pair bow: parallel edges between one pair of nodes each
    /// keep their own dummy chain here, where `edge_points` collapses them.
    /// Empty for a self-loop.
    pub edge_polylines: Vec<Vec<(f64, f64)>>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub layer_gap: f64,
    pub node_gap: f64,
    pub max_crossing_iter: usize,
    pub max_coord_iter: usize,
    /// Pull every source (a node with no in-edges) down to just above its
    /// nearest successor instead of leaving it on the first layer, so a
    /// source feeding a deep node doesn't drag a long edge across the
    /// whole diagram.
    pub pull_sources: bool,
    /// After the barycenter sweeps, swap adjacent nodes in a layer while
    /// that removes crossings (the classic "transpose" refinement).
    pub transpose: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            layer_gap: 80.0,
            node_gap: 40.0,
            max_crossing_iter: 24,
            max_coord_iter: 24,
            pull_sources: false,
            transpose: false,
        }
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum LayoutError {
    #[error("unknown node id {0}")]
    UnknownNode(NodeId),
    #[error("missing size for node {0}")]
    MissingSize(NodeId),
    #[error("duplicate node id {0}")]
    DuplicateNode(NodeId),
    #[error("pinned node {0} has an edge pointing into its pinned side")]
    PinnedAgainstEdge(NodeId),
}

mod coord;
mod cycle;
mod layer;
mod order;
mod route;
mod work;

#[cfg(test)]
mod tests;

/// Nodes forced onto the outermost layers: `first` onto a dedicated layer
/// above every other node, `last` onto one below every other node. Lets a
/// caller anchor "port" nodes at a cluster's border so edges leaving the
/// cluster are routed through its layers instead of through its nodes.
/// A pinned node may only have edges pointing away from the pinned side
/// (`first`: out-edges only, `last`: in-edges only); `layout_pinned`
/// rejects anything else with [`LayoutError::PinnedAgainstEdge`]. Checking
/// the input is enough: such a node is a pure source/sink, so it lies on no
/// cycle and cycle removal never reverses one of its edges.
///
/// `order` fixes the left-to-right order of pinned nodes: a pinned layer
/// whose nodes all have a key is sorted by it and left alone by crossing
/// minimisation, which then arranges everything else around it.
#[derive(Debug, Clone, Default)]
pub struct LayerPins {
    pub first: Vec<NodeId>,
    pub last: Vec<NodeId>,
    pub order: HashMap<NodeId, f64>,
}

pub fn layout_with(g: &Graph, cfg: &LayoutConfig) -> Result<Layout, LayoutError> {
    layout_pinned(g, cfg, &LayerPins::default())
}

pub fn layout_pinned(
    g: &Graph,
    cfg: &LayoutConfig,
    pins: &LayerPins,
) -> Result<Layout, LayoutError> {
    let mut w = work::Work::from_input(g)?;
    let first = pin_indices(&w, &pins.first)?;
    let last = pin_indices(&w, &pins.last)?;
    let (first_ids, last_ids): (HashSet<NodeId>, HashSet<NodeId>) = (
        pins.first.iter().copied().collect(),
        pins.last.iter().copied().collect(),
    );
    for &(u, v) in &g.edges {
        if first_ids.contains(&v) {
            return Err(LayoutError::PinnedAgainstEdge(v));
        }
        if last_ids.contains(&u) {
            return Err(LayoutError::PinnedAgainstEdge(u));
        }
    }
    for (id, &key) in &pins.order {
        let i = *w.real_idx.get(id).ok_or(LayoutError::UnknownNode(*id))?;
        w.order_key[i] = Some(key);
    }
    cycle::remove(&mut w);
    layer::assign_pinned(&mut w, &first, &last, cfg.pull_sources);
    order::minimize_crossings(&mut w, cfg.max_crossing_iter, cfg.transpose);
    coord::assign(&mut w, cfg);
    Ok(route::build(&w))
}

fn pin_indices(w: &work::Work, ids: &[NodeId]) -> Result<Vec<usize>, LayoutError> {
    ids.iter()
        .map(|id| {
            w.real_idx
                .get(id)
                .copied()
                .ok_or(LayoutError::UnknownNode(*id))
        })
        .collect()
}
