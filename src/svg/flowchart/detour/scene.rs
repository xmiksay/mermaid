//! The obstacles of one diagram — nodes and markers — indexed so a curve
//! only tests what is near it.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use super::super::route::Pt;
use super::body::Body;
use super::index::Layer;
use super::DetourEdge;
use crate::svg::builder::curve_basis_samples;

/// How close a re-aimed end may come to another edge's head.
const HEAD_ROOM: f64 = 6.0;

/// One body a curve runs through: its index in the scene and the first and
/// last sample segments inside it.
pub(super) struct Hit {
    pub(super) ob: Ob,
    pub(super) first: usize,
    pub(super) last: usize,
}

/// Segments looked up together.
pub(super) const CHUNK: usize = 16;
/// No owner: a node.
const NONE: usize = usize::MAX;

/// The last, shorter run of segments `windows(CHUNK + 1).step_by(CHUNK)`
/// leaves out (all of them when there are fewer than `CHUNK`).
pub(super) fn tail(samples: &[Pt]) -> Option<&[Pt]> {
    let segs = samples.len().checked_sub(1)?;
    let done = segs / CHUNK * CHUNK;
    (done < segs).then(|| samples.get(done..)).flatten()
}

pub(super) fn bbox(pts: &[Pt]) -> [f64; 4] {
    let inf = f64::INFINITY;
    pts.iter().fold([inf, inf, -inf, -inf], |m, p| {
        [m[0].min(p.0), m[1].min(p.1), m[2].max(p.0), m[3].max(p.1)]
    })
}

pub(super) fn segment_box(a: Pt, b: Pt, m: f64) -> [f64; 4] {
    [
        a.0.min(b.0) - m,
        a.1.min(b.1) - m,
        a.0.max(b.0) + m,
        a.1.max(b.1) + m,
    ]
}

/// Every obstacle — nodes, then markers — with the id of the node it
/// belongs to.
pub(in super::super) struct Scene<'a> {
    nodes: Layer<(&'a str, Body)>,
    marks: Layer<(&'a str, Body)>,
    /// `near_any` answers by box, until the markers move: a search asks
    /// the same of every candidate's unchanged beziers.
    near: RefCell<HashMap<[u64; 4], bool>>,
}

/// Closer than this to a node, a curve all but touches it.
pub(super) const CLOSE: f64 = 5.0;

/// What a curve touches: nodes cut, nodes grazed (and of those, the ones
/// come `CLOSE`), markers cut.
#[derive(Default)]
pub(super) struct Touch {
    pub(super) cut: Vec<Ob>,
    pub(super) grazed: Vec<Ob>,
    pub(super) close: Vec<Ob>,
    pub(super) marks: Vec<Ob>,
}

/// An obstacle in a scene: `(true, i)` the `i`-th node, `(false, i)` the
/// `i`-th marker.
pub(super) type Ob = (bool, usize);

impl<'a> Scene<'a> {
    pub(super) fn new(nodes: Vec<(&'a str, Body)>, marks: Vec<(usize, (&'a str, Body))>) -> Self {
        Self {
            nodes: Layer::new(nodes.into_iter().map(|n| (NONE, n)).collect(), |n| n.1.bbox),
            marks: Layer::new(marks, |m| m.1.bbox),
            near: RefCell::default(),
        }
    }

    pub(super) fn get(&self, (node, i): Ob) -> Option<&(&'a str, Body)> {
        if node {
            self.nodes.items.get(i)
        } else {
            self.marks.items.get(i)
        }
    }

    /// The nodes overlapping a node `edge` ends on (a layout that stacks
    /// nodes leaves no way past them): the edge may run through those.
    pub(super) fn stacked(&self, edge: &DetourEdge) -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut near = Vec::new();
        for (end, id) in edge.clips.iter().zip(edge.ends) {
            if end.shape.is_none() {
                continue;
            }
            let (c, h) = (end.center, (end.size.0 / 2.0 - 1.0, end.size.1 / 2.0 - 1.0));
            let b = [c.0 - h.0, c.1 - h.1, c.0 + h.0, c.1 + h.1];
            self.nodes.near(&b, &mut near);
            for &i in &near {
                if let Some(&(other, body)) = self.nodes.items.get(i) {
                    if other != id && body.overlaps(&b) && !out.contains(&other) {
                        out.push(other);
                    }
                }
            }
        }
        out
    }

    /// Whether any node but those `skip` names overlaps the box `b`.
    pub(super) fn may_hit(&self, b: &[f64; 4], skip: impl Fn(&str, Pt) -> bool) -> bool {
        let mut near = Vec::new();
        self.nodes.near(b, &mut near);
        near.iter().any(|&i| {
            self.nodes
                .items
                .get(i)
                .is_some_and(|(id, body)| body.overlaps(b) && !skip(id, body.c))
        })
    }

    /// Calls `f` with each node (and with `marks` marker) the sampled
    /// curve runs through, grown by `grow`, but those `skip` names, and the
    /// sample segment it does so on, until `f` returns false.
    fn scan(
        &self,
        samples: &[Pt],
        marks: bool,
        grow: f64,
        skip: impl Fn(&str, Pt) -> bool,
        mut f: impl FnMut(Ob, usize) -> bool,
    ) {
        let mut near = Vec::new();
        let layers: &[(bool, &Layer<(&str, Body)>)] = if marks {
            &[(true, &self.nodes), (false, &self.marks)]
        } else {
            &[(true, &self.nodes)]
        };
        // Look bodies up once per run of segments, then test each segment
        // only against those overlapping the run.
        let runs = samples
            .windows(CHUNK + 1)
            .step_by(CHUNK)
            .chain(tail(samples));
        for (c, run) in runs.enumerate() {
            let b = bbox(run);
            let b = [b[0] - grow, b[1] - grow, b[2] + grow, b[3] + grow];
            for &(kind, layer) in layers {
                layer.near(&b, &mut near);
                near.retain(|&i| {
                    layer
                        .items
                        .get(i)
                        .is_some_and(|(_, body)| body.overlaps(&b))
                });
                near.sort_unstable();
                near.dedup();
                for &i in &near {
                    let Some(&(id, body)) = layer.items.get(i) else {
                        continue;
                    };
                    let body = if grow > 0.0 { body.grown(grow) } else { body };
                    for (k, w) in run.windows(2).enumerate() {
                        if body.cuts(w[0], w[1])
                            && !skip(id, body.c)
                            && !f((kind, i), c * CHUNK + k)
                        {
                            return;
                        }
                    }
                }
            }
        }
    }

    /// The nodes — and with `marks` the markers — the sampled curve runs
    /// through, grown by `grow`, but those `skip` names, in the order the
    /// curve reaches them.
    pub(super) fn hits(
        &self,
        samples: &[Pt],
        marks: bool,
        grow: f64,
        skip: impl Fn(&str, Pt) -> bool,
    ) -> Vec<Hit> {
        let mut found: BTreeMap<Ob, (usize, usize)> = BTreeMap::new();
        self.scan(samples, marks, grow, skip, |ob, seg| {
            let e = found.entry(ob).or_insert((seg, seg));
            *e = (e.0.min(seg), e.1.max(seg));
            true
        });
        let mut out: Vec<Hit> = found
            .into_iter()
            .map(|(ob, (first, last))| Hit { ob, first, last })
            .collect();
        out.sort_by_key(|h| (h.first, h.ob));
        out
    }

    /// In one pass: the nodes the sampled curve cuts, the nodes it comes
    /// within `grow` of, and within `CLOSE` of, and the markers it cuts —
    /// but those `skip` names.
    pub(super) fn touches(
        &self,
        samples: &[Pt],
        grow: f64,
        skip: impl Fn(&str, Pt) -> bool,
    ) -> Touch {
        let mut t = Touch::default();
        let mut near = Vec::new();
        let runs = samples
            .windows(CHUNK + 1)
            .step_by(CHUNK)
            .chain(tail(samples));
        for run in runs {
            let b = bbox(run);
            let g = [b[0] - grow, b[1] - grow, b[2] + grow, b[3] + grow];
            for (kind, layer, area) in [(true, &self.nodes, g), (false, &self.marks, b)] {
                layer.near(&area, &mut near);
                near.sort_unstable();
                near.dedup();
                for &i in &near {
                    let Some(&(id, body)) = layer.items.get(i) else {
                        continue;
                    };
                    if !body.overlaps(&area) || skip(id, body.c) {
                        continue;
                    }
                    let (wide, close) = (body.grown(grow), body.grown(CLOSE.min(grow)));
                    for w in run.windows(2) {
                        if kind && wide.cuts(w[0], w[1]) {
                            t.grazed.push((kind, i));
                            if close.cuts(w[0], w[1]) {
                                t.close.push((kind, i));
                            }
                        }
                        if body.cuts(w[0], w[1]) {
                            if kind { &mut t.cut } else { &mut t.marks }.push((kind, i));
                        }
                    }
                }
            }
        }
        for v in [&mut t.cut, &mut t.grazed, &mut t.close, &mut t.marks] {
            v.sort_unstable();
            v.dedup();
        }
        t
    }

    /// Whether the sampled curve runs through anything (as `hits`),
    /// stopping at the first.
    pub(super) fn hits_any(
        &self,
        samples: &[Pt],
        marks: bool,
        grow: f64,
        skip: impl Fn(&str, Pt) -> bool,
    ) -> bool {
        let mut other = false;
        self.scan(samples, marks, grow, skip, |_, _| {
            other = true;
            false
        });
        other
    }

    /// Whether the end `which` of `edge`, drawn through `ctrl`, keeps out of
    /// every node it does not own (the few spans near that end).
    pub(in super::super) fn keeps_clear(
        &self,
        edge: &DetourEdge,
        ctrl: &[Pt],
        which: usize,
    ) -> bool {
        const NEAR_END: usize = 6;
        let n = ctrl.len();
        let part = if which == 0 {
            ctrl.get(..NEAR_END.min(n))
        } else {
            ctrl.get(n.saturating_sub(NEAR_END)..)
        };
        let samples = curve_basis_samples(part.unwrap_or(ctrl), 16);
        let stacked = self.stacked(edge);
        !self.hits_any(&samples, false, 0.0, |id, c| {
            edge.owns(id, c) || stacked.contains(&id)
        })
    }

    /// Whether anything — node or marker, owned or not — overlaps the box
    /// `b` grown by `m`.
    pub(super) fn near_any(&self, b: &[f64; 4], m: f64) -> bool {
        let b = [b[0] - m, b[1] - m, b[2] + m, b[3] + m];
        let key = b.map(f64::to_bits);
        if let Some(&v) = self.near.borrow().get(&key) {
            return v;
        }
        let mut near = Vec::new();
        let v = [&self.nodes, &self.marks].iter().any(|layer| {
            layer.near(&b, &mut near);
            near.iter().any(|&i| {
                layer
                    .items
                    .get(i)
                    .is_some_and(|(_, body)| body.overlaps(&b))
            })
        });
        self.near.borrow_mut().insert(key, v);
        v
    }

    /// Whether `p` lies on (or within `HEAD_ROOM` of) a marker at the node
    /// `id` of an edge other than `edge`: heads of different kinds there
    /// would run together.
    pub(super) fn head_at(&self, id: &str, p: Pt, edge: usize) -> bool {
        let mut near = Vec::new();
        self.marks.near(&segment_box(p, p, HEAD_ROOM), &mut near);
        near.iter().any(|&i| {
            self.marks.owner.get(i).is_some_and(|&o| o != edge)
                && self
                    .marks
                    .items
                    .get(i)
                    .is_some_and(|(m, b)| *m == id && b.grown(HEAD_ROOM).contains(p))
        })
    }

    /// `edge` moved: its markers are now these.
    pub(super) fn moved(&mut self, edge: usize, marks: Vec<(&'a str, Body)>) {
        self.marks.replace(edge, marks);
        self.near.get_mut().clear();
    }
}

#[cfg(test)]
mod tests;
