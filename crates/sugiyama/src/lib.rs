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

use std::collections::HashMap;
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
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub layer_gap: f64,
    pub node_gap: f64,
    pub max_crossing_iter: usize,
    pub max_coord_iter: usize,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            layer_gap: 80.0,
            node_gap: 40.0,
            max_crossing_iter: 24,
            max_coord_iter: 24,
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
}

mod coord;
mod cycle;
mod layer;
mod order;
mod route;
mod work;

pub fn layout(graph: &Graph) -> Result<Layout, LayoutError> {
    layout_with(graph, &LayoutConfig::default())
}

pub fn layout_with(g: &Graph, cfg: &LayoutConfig) -> Result<Layout, LayoutError> {
    let mut w = work::Work::from_input(g)?;
    cycle::remove(&mut w);
    layer::assign(&mut w);
    order::minimize_crossings(&mut w, cfg.max_crossing_iter);
    coord::assign(&mut w, cfg);
    Ok(route::build(&w))
}
