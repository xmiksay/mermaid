//! Graph mode: the layered (`sugiyama`) layout for diagrams with many
//! relations or an explicit `LAYOUT_*()` direction.
//!
//! Boundaries are laid out as compound clusters, bottom-up: each boundary's
//! direct items (shapes and already-laid-out child boundaries) get their own
//! layered pass, and the resulting frame then takes part in its parent's pass
//! as a single node. So a frame always encloses exactly its members, and
//! sibling frames and foreign shapes can never overlap it — by construction,
//! not by post-hoc nudging (flowchart's `resolve_subgraph_overlaps` only
//! separates sibling frames, which is not enough here).
//!
//! A relation leaving a boundary is routed inside that boundary towards a
//! *port*: a point-sized node pinned onto the boundary's first or last layer,
//! on the side facing the relation's other end, so the inner pass threads it
//! between the members (via its dummy nodes) to the frame edge instead of
//! straight through them. `route::assemble` stitches the per-level pieces.
//!
//! Sugiyama only lays out top-down; left-right swaps every size going in and
//! every coordinate coming out (the flowchart `LR` transform).

use std::collections::HashMap;

use crate::parse::{C4BoundaryKind, C4Diagram, C4LayoutDirection, C4Relation};
use crate::sugiyama::{LayoutConfig, LayoutError};

use super::outline::{Rect, Span};
use super::rel_loop::enclosure_room;
use super::rel_loop::loop_room;
use super::text::bundle_label_extent;
use super::{header_min_w, BoundaryBox, Placement};

mod ends;
mod frames;
mod legs;
mod orient;
mod pass;
mod route;
mod runs;
#[cfg(test)]
mod tests;
mod tree;

use ends::{ease_ends, snap_slivers, spread_ends, Scenery};
use legs::bow_apart;
use pass::Pass;
use route::assemble;
use tree::{rel_path, RelPath, Tree};

type Pt = (f64, f64);

/// Minimum gap between layers; widened to fit the tallest (TD) or widest (LR)
/// relation label so labels have room between the shapes they connect.
const MIN_LAYER_GAP: f64 = 70.0;
/// Room beside a label in a layer gap: clearances plus an arrowhead.
const LABEL_LAYER_ROOM: f64 = 60.0;
/// Room for staggered turns: a base plus one `TRACK_GAP` per line fanning
/// out of the busiest shape, counting at most `MAX_TRACKS` lines.
const FAN_BASE: f64 = 30.0;
const MAX_TRACKS: usize = 6;
pub(super) const TRACK_GAP: f64 = 12.0;
/// Gap between stacked labels of one bundle.
const LABEL_STACK_GAP: f64 = 12.0;
const NODE_GAP_TD: f64 = 50.0;
const NODE_GAP_LR: f64 = 40.0;

#[derive(Clone, Copy)]
pub(super) struct Dir {
    lr: bool,
}

impl Dir {
    /// Swap between screen space and sugiyama's top-down space (an
    /// involution, so the same call converts both ways).
    fn swap(self, p: Pt) -> Pt {
        if self.lr {
            (p.1, p.0)
        } else {
            p
        }
    }

    fn rect(self, r: Rect) -> Rect {
        let (x, y) = self.swap((r.0, r.1));
        let (w, h) = self.swap((r.2, r.3));
        (x, y, w, h)
    }
}

/// A cluster laid out in its own top-down coordinate space.
pub(super) struct Local {
    /// Frame around the items, including boundary padding.
    frame: Span,
    /// The boundary's header text block, which routes must not cross.
    header: Option<Span>,
    centers: Vec<Pt>,
    sizes: Vec<Pt>,
    /// Relation index → its piece at this level, in relation order: from
    /// the source item (or the port it enters by) to the target item (or the
    /// port it leaves by).
    rel_pts: HashMap<usize, Vec<Pt>>,
}

impl Local {
    fn size(&self) -> Pt {
        (self.frame.x1 - self.frame.x0, self.frame.y1 - self.frame.y0)
    }
}

/// Lay `d` out in graph mode; a layout error (which the construction rules
/// out) is reported so the caller can fall back to the grid.
pub(super) fn graph_placement(d: &C4Diagram, font_size: f64) -> Result<Placement, LayoutError> {
    let dir = Dir {
        lr: d.layout_direction == Some(C4LayoutDirection::LeftRight),
    };
    let tree = Tree::build(&d.elements);
    let paths: Vec<Option<RelPath>> = d.relations.iter().map(|r| rel_path(&tree, r)).collect();
    let pass = Pass {
        tree: &tree,
        paths: &paths,
        rels: &d.relations,
        dir,
        cfg: layout_config(&d.relations, dir),
        font_size,
        loops: loop_room(&d.relations, dir.lr),
        enclosures: enclosure_room(&d.elements, &d.relations, dir.lr),
    };
    let (locals, offset) = pass.settle()?;

    let mut out = Placement {
        curved: true,
        narrow_labels: dir.lr,
        ..Placement::default()
    };
    emit(&tree, 0, &locals, &offset, dir, font_size, &mut out);

    let mut marks = route::FrameMarks::default();
    let mut routes: Vec<Option<Vec<Pt>>> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| assemble(p.as_ref()?, i, &locals, &offset, dir.lr, &mut marks))
        .collect();
    let shape = |alias: &str| {
        let &(c, i) = tree.home.get(alias)?;
        tree.clusters[c].opens[i]
            .is_none()
            .then(|| ((c, i), locals[c].sizes[i].0))
    };
    let ends: Vec<_> = d
        .relations
        .iter()
        .map(|r| [shape(&r.from), shape(&r.to)])
        .collect();
    let shapes: Vec<((usize, usize), Rect)> = (0..tree.clusters.len())
        .flat_map(|c| {
            let (locals, offset, tree) = (&locals, &offset, &tree);
            (0..tree.clusters[c].items.len())
                .filter(move |&i| tree.clusters[c].opens[i].is_none())
                .map(move |i| {
                    let (cx, cy) = add(locals[c].centers[i], offset[c]);
                    let (w, h) = locals[c].sizes[i];
                    ((c, i), (cx - w / 2.0, cy - h / 2.0, w, h))
                })
        })
        .collect();
    // A relation ending on a boundary is routed to a port among its members:
    // those members are not in the way.
    let opens = |alias: &str| {
        let &(c, i) = tree.home.get(alias)?;
        tree.clusters[c].opens[i]
    };
    let under = |mut c: usize, cc: usize| loop {
        if c == cc {
            break true;
        }
        match tree.clusters[c].parent {
            Some(p) => c = p,
            None => break false,
        }
    };
    let skip: Vec<Vec<(usize, usize)>> = d
        .relations
        .iter()
        .map(|r| {
            let frames: Vec<usize> = [&r.from, &r.to]
                .into_iter()
                .filter_map(|a| opens(a))
                .collect();
            shapes
                .iter()
                .filter(|((c, _), _)| frames.iter().any(|&cc| under(*c, cc)))
                .map(|&(k, _)| k)
                .collect()
        })
        .collect();
    let frames: Vec<Span> = (0..tree.clusters.len())
        .filter(|&c| tree.clusters[c].boundary.is_some())
        .map(|c| locals[c].frame.shift(offset[c]))
        .collect();
    let headers: Vec<Rect> = (0..tree.clusters.len())
        .filter_map(|c| Some(locals[c].header?.shift(offset[c]).to_rect()))
        .collect();
    let scenery = Scenery {
        shapes: &shapes,
        skip: &skip,
        frames: &frames,
        headers: &headers,
    };
    spread_ends(&mut routes, &ends, &scenery);
    let rects: Vec<Rect> = shapes.iter().map(|&(_, r)| r).collect();
    bow_apart(&mut routes, &rects);
    out.routes = routes
        .into_iter()
        .map(|r| {
            let eased = snap_slivers(ease_ends(&r?));
            Some(eased.into_iter().map(|p| dir.swap(p)).collect())
        })
        .collect();
    Ok(out)
}

/// Emit cluster `c`'s shapes and boundary frames in declaration (pre-)order,
/// the order `rel::endpoint_shapes` relies on to resolve duplicate aliases.
fn emit(
    tree: &Tree,
    c: usize,
    locals: &[Local],
    offset: &[Pt],
    dir: Dir,
    font_size: f64,
    out: &mut Placement,
) {
    let cluster = &tree.clusters[c];
    for (i, el) in cluster.items.iter().enumerate() {
        if let Some(cc) = cluster.opens[i] {
            let frame = locals[cc].frame.shift(offset[cc]);
            out.boundaries.push(BoundaryBox {
                alias: el.alias.clone(),
                label: el.label.clone(),
                kind: el.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
                type_text: el.boundary_type.clone(),
                rect: dir.rect(frame.to_rect()),
                header_w: header_min_w(el, font_size) - 28.0,
            });
            emit(tree, cc, locals, offset, dir, font_size, out);
        } else {
            let (cx, cy) = add(locals[c].centers[i], offset[c]);
            let (w, h) = locals[c].sizes[i];
            out.leaves
                .push(((*el).clone(), dir.rect((cx - w / 2.0, cy - h / 2.0, w, h))));
        }
    }
}

fn layout_config(rels: &[C4Relation], dir: Dir) -> LayoutConfig {
    // A bundle of relations between one pair stacks its labels along the
    // lines, so the gap must fit all of them.
    let (w, h) = bundle_label_extent(rels, dir.lr, LABEL_STACK_GAP);
    let label_extent = if dir.lr { w } else { h };
    // Lines fanning out of one shape turn at staggered depths
    // (`ends::spread_ends`), a track apart: the gap must hold them.
    let mut fan: HashMap<&str, usize> = HashMap::new();
    for r in rels.iter().filter(|r| r.from != r.to) {
        *fan.entry(r.from.as_str()).or_insert(0) += 1;
    }
    let fan_out = fan.values().copied().max().unwrap_or(0).min(MAX_TRACKS);
    let track_room = FAN_BASE + TRACK_GAP * fan_out as f64;
    LayoutConfig {
        layer_gap: (label_extent + LABEL_LAYER_ROOM)
            .max(MIN_LAYER_GAP)
            .max(track_room),
        node_gap: if dir.lr { NODE_GAP_LR } else { NODE_GAP_TD },
        pull_sources: true,
        transpose: true,
        max_crossing_iter: 48,
        ..LayoutConfig::default()
    }
}

fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}
