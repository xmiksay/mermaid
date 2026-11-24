//! One layout pass over every cluster, and settling the port sides.

use std::collections::HashMap;

use crate::parse::C4Relation;
use crate::sugiyama::{layout_pinned, Graph, LayerPins, LayoutConfig, LayoutError, NodeId};

use super::super::text::shape_size;
use super::ends::{END_INSET, MIN_END_GAP};
use super::frames::{boundary_frame, items_bbox};
use super::orient::backward_edges;
use super::route::{leave_bands, through_bands};
use super::tree::{RelPath, Tree};
use super::{add, Dir, Local, Pt};

/// Size of a port node — effectively a point, so it adds no width to a layer.
const PORT_SIZE: (f64, f64) = (1.0, 0.0);
/// Upper bound on layout passes while settling port sides. A side that has
/// flipped twice is frozen, so settling normally ends well before this.
const MAX_SIDE_PASSES: usize = 6;

/// Whether the port of relation `.1` in cluster `.0` sits on the cluster's
/// top (first-layer) side.
pub(super) type PortSides = HashMap<(usize, usize), bool>;
/// Where the relation's other end lies across the flow (top-down x, root
/// space): ports on one side of a frame are ordered by it, so lines leave
/// the frame already in the order they need outside it.
pub(super) type PortKeys = HashMap<(usize, usize), f64>;

pub(super) struct Pass<'p, 'a> {
    pub tree: &'p Tree<'a>,
    pub paths: &'p [Option<RelPath>],
    pub rels: &'p [C4Relation],
    pub dir: Dir,
    pub cfg: LayoutConfig,
    pub font_size: f64,
    /// Room reserved for self-relation loops, by alias (`loop_room`).
    pub loops: HashMap<&'p str, f64>,
    /// Extra bottom padding per boundary (`enclosure_room`).
    pub enclosures: HashMap<&'p str, f64>,
}

/// Every cluster laid out, and each cluster's translation into root space.
pub(super) type Settled = (Vec<Local>, Vec<Pt>);

impl Pass<'_, '_> {
    /// A port belongs on the side of its boundary facing the relation's
    /// other end, which is only known once the parent level is laid out.
    /// Port sides change the inner layering (their dummies take slots), so a
    /// frame's size — and with it the parent's arrangement — can shift:
    /// re-run until the sides the layout reveals are the sides it was given.
    /// A side that keeps flipping is frozen after its second flip, so this
    /// always ends within `MAX_SIDE_PASSES`. Port order keys come from the
    /// previous pass; the final layout always uses some.
    pub fn settle(&self) -> Result<Settled, LayoutError> {
        self.settle_checked().map(|(settled, ..)| settled)
    }

    /// [`Pass::settle`], also returning the port sides the layout was made
    /// with and whether they are the ones it reveals (frozen sides aside).
    pub fn settle_checked(&self) -> Result<(Settled, PortSides, bool), LayoutError> {
        let mut sides = self.default_sides();
        let mut keys = PortKeys::new();
        let mut flips: HashMap<(usize, usize), u8> = HashMap::new();
        let (mut locals, mut offset) = self.run(&sides, &keys)?;
        let mut keyed = sides.is_empty();
        let mut converged = false;
        for _ in 0..MAX_SIDE_PASSES {
            let (mut observed, observed_keys) = self.port_sides(&locals, &offset);
            for (k, top) in observed.iter_mut() {
                let frozen = flips.get(k).copied().unwrap_or(0) >= 2;
                if frozen {
                    if let Some(&was) = sides.get(k) {
                        *top = was;
                    }
                }
            }
            if observed == sides && keyed {
                converged = true;
                break;
            }
            for (k, top) in &observed {
                if sides.get(k) != Some(top) {
                    *flips.entry(*k).or_insert(0) += 1;
                }
            }
            sides = observed;
            keys = observed_keys;
            keyed = true;
            (locals, offset) = self.run(&sides, &keys)?;
        }
        Ok(((locals, offset), sides, converged))
    }

    /// First guess: outgoing relations leave at the bottom, incoming ones
    /// enter at the top.
    fn default_sides(&self) -> PortSides {
        let mut sides = PortSides::new();
        for (i, path) in self.paths.iter().enumerate() {
            let Some(path) = path else { continue };
            for &c in &path.src_side {
                sides.insert((c, i), false);
            }
            for &c in &path.dst_side {
                sides.insert((c, i), true);
            }
        }
        sides
    }

    /// For every port, whether the relation's other end lies above the
    /// port's boundary (in top-down space) — i.e. whether it belongs on top —
    /// and where it lies across the flow.
    pub fn port_sides(&self, locals: &[Local], offset: &[Pt]) -> (PortSides, PortKeys) {
        let center_of = |alias: &str| {
            let &(c, i) = self.tree.home.get(alias)?;
            Some(add(locals[c].centers[i], offset[c]))
        };
        let frame_mid_y = |c: usize| (locals[c].frame.y0 + locals[c].frame.y1) / 2.0 + offset[c].1;
        let mut sides = PortSides::new();
        let mut keys = PortKeys::new();
        for (i, (path, r)) in self.paths.iter().zip(self.rels).enumerate() {
            let Some(path) = path else { continue };
            for (side, other) in [(&path.src_side, &r.to), (&path.dst_side, &r.from)] {
                let Some((ox, oy)) = center_of(other) else {
                    continue;
                };
                for &c in side {
                    sides.insert((c, i), oy < frame_mid_y(c));
                    keys.insert((c, i), ox);
                }
            }
        }
        (sides, keys)
    }

    /// Length a shape's in/out face (its top-down width) needs so that, even
    /// if every relation touching it ended on one face, the ends stay
    /// `MIN_END_GAP` apart. Only bites in left-right layouts, where that face
    /// is the shape's (short) screen height; sizing only for the busier face
    /// was tried and cost crossings, as ends do not split cleanly by
    /// direction.
    fn face_need(&self, alias: &str) -> f64 {
        let ends = self
            .rels
            .iter()
            .filter(|r| r.from != r.to && (r.from == alias || r.to == alias))
            .count();
        2.0 * END_INSET + ends.saturating_sub(1) as f64 * MIN_END_GAP
    }

    /// Lay out every cluster bottom-up, then translate each into root space.
    fn run(
        &self,
        sides: &PortSides,
        keys: &PortKeys,
    ) -> Result<(Vec<Local>, Vec<Pt>), LayoutError> {
        let tree = self.tree;
        // Children have larger indices than their parents, so a reverse
        // sweep lays out every child before the parent that needs its size.
        let n = tree.clusters.len();
        let mut locals: Vec<Option<Local>> = (0..n).map(|_| None).collect();
        for c in (0..n).rev() {
            locals[c] = Some(self.layout_cluster(c, &locals, sides, keys)?);
        }
        let locals: Vec<Local> = locals.into_iter().flatten().collect();

        let mut offset = vec![(0.0, 0.0); n];
        offset[0] = (-locals[0].frame.x0, -locals[0].frame.y0);
        for c in 0..n {
            for (i, opens) in tree.clusters[c].opens.iter().enumerate() {
                let Some(cc) = *opens else { continue };
                let center = add(locals[c].centers[i], offset[c]);
                let (w, h) = locals[cc].size();
                offset[cc] = (
                    center.0 - w / 2.0 - locals[cc].frame.x0,
                    center.1 - h / 2.0 - locals[cc].frame.y0,
                );
            }
        }
        Ok((locals, offset))
    }

    fn layout_cluster(
        &self,
        c: usize,
        locals: &[Option<Local>],
        sides: &PortSides,
        keys: &PortKeys,
    ) -> Result<Local, LayoutError> {
        let (tree, dir) = (self.tree, self.dir);
        let cluster = &tree.clusters[c];
        let sizes: Vec<Pt> = cluster
            .items
            .iter()
            .zip(&cluster.opens)
            .map(
                |(el, opens)| match opens.and_then(|cc| locals[cc].as_ref()) {
                    Some(child) => child.size(),
                    None => {
                        let (w, h) = dir.swap(shape_size(el));
                        (w.max(self.face_need(&el.alias)), h)
                    }
                },
            )
            .collect();

        // A self-relation's loop and label need room beside the shape; the
        // layout reserves it (on both sides, as sugiyama centres nodes) while
        // the shape is drawn at its own size.
        let layout_sizes: Vec<Pt> = cluster
            .items
            .iter()
            .zip(&sizes)
            .map(|(el, &(w, h))| {
                let room = self.loops.get(el.alias.as_str()).copied().unwrap_or(0.0);
                let (dw, dh) = dir.swap((2.0 * room, 0.0));
                (w + dw, h + dh)
            })
            .collect();
        let mut node_size: HashMap<NodeId, Pt> = layout_sizes
            .iter()
            .enumerate()
            .map(|(i, &s)| (i as NodeId, s))
            .collect();
        // Per layout edge: the relation it carries and whether its points run
        // against that relation.
        let mut edges: Vec<(NodeId, NodeId)> = Vec::new();
        let mut owners: Vec<(usize, bool)> = Vec::new();
        let mut pins = LayerPins::default();
        let mut next = sizes.len() as NodeId;
        // A port edge always points away from its pinned side so it spans every
        // layer in between.
        let mut port = |item: usize, top: bool, key: Option<f64>| {
            let id = next;
            next += 1;
            node_size.insert(id, PORT_SIZE);
            if let Some(key) = key {
                pins.order.insert(id, key);
            }
            if top {
                pins.first.push(id);
                (id, item as NodeId)
            } else {
                pins.last.push(id);
                (item as NodeId, id)
            }
        };
        for (i, (path, r)) in self.paths.iter().zip(self.rels).enumerate() {
            let Some(path) = path else { continue };
            let (edge, reversed) = if path.common == c {
                match (tree.item_in(&r.from, c), tree.item_in(&r.to, c)) {
                    (Some(u), Some(v)) if u != v => ((u as NodeId, v as NodeId), false),
                    _ => continue,
                }
            } else if path.src_side.contains(&c) {
                let Some(u) = tree.item_in(&r.from, c) else {
                    continue;
                };
                let top = sides.get(&(c, i)).copied().unwrap_or(false);
                (port(u, top, keys.get(&(c, i)).copied()), top)
            } else if path.dst_side.contains(&c) {
                let Some(v) = tree.item_in(&r.to, c) else {
                    continue;
                };
                let top = sides.get(&(c, i)).copied().unwrap_or(true);
                (port(v, top, keys.get(&(c, i)).copied()), !top)
            } else {
                continue;
            };
            edges.push(edge);
            owners.push((i, reversed));
        }

        // Ports are pure sources/sinks, so this only ever flips relation
        // edges between items, never a port edge.
        let as_usize: Vec<(usize, usize)> = edges
            .iter()
            .map(|&(u, v)| (u as usize, v as usize))
            .collect();
        for (k, back) in backward_edges(next as usize, &as_usize)
            .into_iter()
            .enumerate()
        {
            if back {
                edges[k] = (edges[k].1, edges[k].0);
                owners[k].1 = !owners[k].1;
            }
        }

        let g = Graph {
            nodes: (0..next).collect(),
            edges,
            node_size,
        };
        let layout = layout_pinned(&g, &self.cfg, &pins)?;
        let centers: Vec<Pt> = (0..sizes.len() as NodeId)
            .map(|i| layout.node_pos.get(&i).copied().unwrap_or((0.0, 0.0)))
            .collect();
        let leaf: Vec<bool> = cluster.opens.iter().map(Option::is_none).collect();
        let mut rel_pts = HashMap::new();
        for (k, &(rel, reversed)) in owners.iter().enumerate() {
            let Some(pts) = layout.edge_polylines.get(k).filter(|p| p.len() >= 2) else {
                continue;
            };
            let pts = through_bands(&centers, &sizes, pts);
            // A boundary that is the relation's own endpoint (not a block it
            // passes through to a member) is left like a shape: its piece
            // ends at the block's centre, and bending inside the band would
            // cut across whatever else sits in it.
            let r = &self.rels[rel];
            let ends: Vec<bool> = cluster
                .items
                .iter()
                .zip(&leaf)
                .map(|(el, &l)| l || el.alias == r.from || el.alias == r.to)
                .collect();
            let mut pts = leave_bands(&centers, &sizes, &ends, &pts);
            if reversed {
                pts.reverse();
            }
            rel_pts.insert(rel, pts);
        }

        // Ports and dummy waypoints lie inside the frame's cross extent, so
        // every route stays within it and leaves through the top or bottom
        // edge rather than slipping out of a side.
        let port_xs: Vec<f64> = (sizes.len() as NodeId..next)
            .filter_map(|id| layout.node_pos.get(&id).map(|p| p.0))
            .chain(layout.edge_polylines.iter().flat_map(|p| {
                p.iter()
                    .skip(1)
                    .take(p.len().saturating_sub(2))
                    .map(|q| q.0)
            }))
            .collect();
        let (frame, header) = match cluster.boundary {
            Some(b) => {
                let extra = self
                    .enclosures
                    .get(b.alias.as_str())
                    .copied()
                    .unwrap_or(0.0);
                boundary_frame(
                    &centers,
                    &layout_sizes,
                    &port_xs,
                    b,
                    dir,
                    (self.font_size, extra),
                )
            }
            None => (items_bbox(&centers, &layout_sizes), None),
        };
        Ok(Local {
            frame,
            header,
            centers,
            sizes,
            rel_pts,
        })
    }
}
