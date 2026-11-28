//! Detours (#403): an edge whose drawn curve runs through a node it does not
//! end on is steered around that node by one waypoint, or a pair, beside the
//! stretch it crosses, out in the gap between nodes. Every candidate that
//! clears the node without hitting another node or marker, and without
//! running backwards or doubling the edge, is scored: how far it moves the
//! curve aside (so the shorter side wins by default), plus heavy penalties
//! for running along another edge or a frame border and for grazing a node.
//! The waypoints go between the end runs (face point, lead-in and the point
//! beyond), so heads stay square-on and straight under their markers; a
//! round end is aimed again from its new neighbour.

use std::cell::RefCell;

use crate::parse::NodeShape;
use crate::sugiyama::LayoutConfig;

use super::edges::EndClip;
use super::route::{EdgePath, Pt, LEAD_MARGIN};

mod body;
mod cands;
mod index;
mod lines;
mod room;
mod scene;
mod search;

use body::{unit, Body};
use lines::{Lines, FRAME};
pub(super) use scene::Scene;
use search::detour;

/// A node edges may not run through, unless they end on it.
pub(super) struct Obstacle<'a> {
    pub(super) id: &'a str,
    pub(super) node: EndClip,
}

/// What a detour needs of an edge: its end ids, the end shapes (a subgraph
/// end may own the nodes inside its frame) and lead-in lengths (0 for an
/// unmarked end).
pub(super) struct DetourEdge<'a> {
    pub(super) ends: [&'a str; 2],
    pub(super) clips: [EndClip; 2],
    pub(super) leads: [f64; 2],
    /// The flow axis between the ends (0 x, 1 y).
    pub(super) flow: usize,
    /// Its place among the edges (drawing order).
    pub(super) index: usize,
}

/// Whether `c` lies within the box of `e`.
fn within(e: &EndClip, c: Pt) -> bool {
    (c.0 - e.center.0).abs() <= e.size.0 / 2.0 && (c.1 - e.center.1).abs() <= e.size.1 / 2.0
}

impl DetourEdge<'_> {
    /// Whether the node `id` at `c` belongs to an end: is one, or sits
    /// inside a subgraph end's frame that the other end lies inside too (a
    /// member to its own frame, or nested frames) — an edge between a
    /// frame and the outside still keeps off its members.
    fn owns(&self, id: &str, c: Pt) -> bool {
        let [a, b] = &self.clips;
        self.ends.contains(&id)
            || [(a, b), (b, a)]
                .iter()
                .any(|(e, other)| e.shape.is_none() && within(e, other.center) && within(e, c))
    }
}

/// Detour every edge (self-loops excepted) around the nodes it runs through.
/// Other edges' markers, where it does not end itself, are avoided too, but
/// only by a detour: an edge already running over one is not moved for it.
/// `frames` are the subgraph frames `(x0, y0, x1, y1)`. Returns the nodes
/// as obstacles, for what moves the edges after.
pub(super) fn detour_all<'a>(
    paths: &mut [Option<EdgePath>],
    edges: &[Option<DetourEdge<'a>>],
    obstacles: &[Obstacle<'a>],
    frames: &[[f64; 4]],
) -> Scene<'a> {
    let gap = LayoutConfig::default().node_gap;
    let nodes: Vec<(&str, Body)> = obstacles
        .iter()
        .filter_map(|o| Some((o.id, Body::new(&o.node)?)))
        .collect();
    let first_marks = paths
        .iter()
        .zip(edges)
        .enumerate()
        .filter_map(|(i, (p, e))| Some((i, p.as_ref()?, e.as_ref()?)))
        .flat_map(|(i, p, e)| marks(&p.ctrl, e).into_iter().map(move |m| (i, m)))
        .collect();
    let mut scene = Scene::new(nodes, first_marks);
    // Every edge's lines and the frame borders, gathered when a detour
    // first scores running along them, patched as edges move.
    let lines: RefCell<Option<Lines>> = RefCell::new(None);
    for i in 0..paths.len() {
        let (Some(Some(path)), Some(Some(edge))) = (paths.get(i), edges.get(i)) else {
            continue;
        };
        if edge.ends[0] == edge.ends[1] {
            continue;
        }
        let along = |samples: &[Pt]| {
            let mut lines = lines.borrow_mut();
            lines
                .get_or_insert_with(|| all_lines(paths, frames))
                .along(i, samples)
        };
        let Some(ctrl) = detour(&path.ctrl, path.curve, edge, &scene, gap, &along) else {
            continue;
        };
        scene.moved(i, marks(&ctrl, edge));
        if let Some(Some(path)) = paths.get_mut(i) {
            path.label = ctrl.clone();
            path.ctrl = ctrl;
            if let Some(lines) = lines.borrow_mut().as_mut() {
                lines.moved(i, &path.drawn_points());
            }
        }
    }
    scene
}

/// Every edge's drawn lines, and every frame border.
fn all_lines(paths: &[Option<EdgePath>], frames: &[[f64; 4]]) -> Lines {
    let mut segs: Vec<(usize, (Pt, Pt))> = Vec::new();
    for (j, p) in paths.iter().enumerate() {
        if let Some(p) = p {
            let s = p.drawn_points();
            segs.extend(s.windows(2).map(|w| (j, (w[0], w[1]))));
        }
    }
    for f in frames {
        let c = [(f[0], f[1]), (f[2], f[1]), (f[2], f[3]), (f[0], f[3])];
        let sides = c.iter().zip(c.iter().cycle().skip(1));
        segs.extend(sides.map(|(a, b)| (FRAME, (*a, *b))));
    }
    Lines::new(segs)
}

/// The markers of an edge drawn through `ctrl`.
fn marks<'a>(ctrl: &[Pt], edge: &DetourEdge<'a>) -> Vec<(&'a str, Body)> {
    (0..2)
        .filter_map(|which| Some((edge.ends[which], mark(ctrl, edge, which)?)))
        .collect()
}

/// The marker at end `which` of an edge as a disc over its length, with a
/// margin.
fn mark(ctrl: &[Pt], edge: &DetourEdge, which: usize) -> Option<Body> {
    let len = edge.leads[which] - LEAD_MARGIN;
    if len <= 0.0 {
        return None;
    }
    let n = ctrl.len();
    let (tip, next) = if which == 0 {
        (*ctrl.first()?, *ctrl.get(1)?)
    } else {
        (*ctrl.last()?, *ctrl.get(n.checked_sub(2)?)?)
    };
    let u = unit(tip, next)?;
    Body::new(&EndClip {
        center: (tip.0 + u.0 * len / 2.0, tip.1 + u.1 * len / 2.0),
        size: (len + 2.0 * LEAD_MARGIN, len + 2.0 * LEAD_MARGIN),
        shape: Some(NodeShape::Circle),
    })
}

#[cfg(test)]
mod tests;
