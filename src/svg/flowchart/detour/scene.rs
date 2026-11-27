//! The obstacles of one diagram — nodes and markers — indexed so a curve
//! only tests what is near it.

use std::collections::BTreeMap;

use super::super::route::Pt;
use super::body::Body;
use super::index::Index;

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

/// Items with an owning edge, indexed and patched as edges move: a moved
/// edge's old items die and its new ones go into a small second index,
/// folded into the main one once it holds a quarter as many.
struct Layer<T> {
    items: Vec<T>,
    owner: Vec<usize>,
    dead: Vec<bool>,
    area: fn(&T) -> [f64; 4],
    index: Index,
    fresh: Vec<usize>,
    fresh_index: Index,
}

impl<T> Layer<T> {
    fn new(items: Vec<(usize, T)>, area: fn(&T) -> [f64; 4]) -> Self {
        let (owner, items): (Vec<usize>, Vec<T>) = items.into_iter().unzip();
        let all: Vec<usize> = (0..items.len()).collect();
        let mut layer = Self {
            dead: vec![false; items.len()],
            items,
            owner,
            area,
            index: Index::new(&[]),
            fresh: Vec::new(),
            fresh_index: Index::new(&[]),
        };
        layer.index = layer.index_of(&all);
        layer
    }

    fn index_of(&self, ids: &[usize]) -> Index {
        let boxes: Vec<(usize, [f64; 4])> = ids
            .iter()
            .filter(|&&i| !self.dead.get(i).copied().unwrap_or(true))
            .filter_map(|&i| Some((i, (self.area)(self.items.get(i)?))))
            .collect();
        Index::new(&boxes)
    }

    /// Live items that may overlap `b`, possibly repeated.
    fn near(&self, b: &[f64; 4], out: &mut Vec<usize>) {
        out.clear();
        for (index, all) in [(&self.index, None), (&self.fresh_index, Some(&self.fresh))] {
            let start = out.len();
            if index.near(b, out).is_none() {
                out.truncate(start);
                match all {
                    Some(fresh) => out.extend(fresh),
                    None => out.extend(0..self.items.len()),
                }
            }
        }
        out.retain(|&i| !self.dead.get(i).copied().unwrap_or(true));
    }

    fn replace(&mut self, edge: usize, new: Vec<T>) {
        for (o, d) in self.owner.iter().zip(&mut self.dead) {
            *d |= *o == edge;
        }
        for t in new {
            self.fresh.push(self.items.len());
            self.items.push(t);
            self.owner.push(edge);
            self.dead.push(false);
        }
        if self.fresh.len() * 4 > self.items.len() {
            let all: Vec<usize> = (0..self.items.len()).collect();
            self.index = self.index_of(&all);
            self.fresh.clear();
        }
        self.fresh_index = self.index_of(&self.fresh);
    }
}

/// Every obstacle — nodes, then markers — with the id of the node it
/// belongs to.
pub(super) struct Scene<'a> {
    nodes: Layer<(&'a str, Body)>,
    marks: Layer<(&'a str, Body)>,
}

/// What a curve touches: nodes cut, nodes grazed, markers cut.
#[derive(Default)]
pub(super) struct Touch {
    pub(super) cut: Vec<Ob>,
    pub(super) grazed: Vec<Ob>,
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
        }
    }

    pub(super) fn get(&self, (node, i): Ob) -> Option<&(&'a str, Body)> {
        if node {
            self.nodes.items.get(i)
        } else {
            self.marks.items.get(i)
        }
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
    /// within `grow` of, and the markers it cuts — but those `skip` names.
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
                    let wide = body.grown(grow);
                    for w in run.windows(2) {
                        if kind && wide.cuts(w[0], w[1]) {
                            t.grazed.push((kind, i));
                        }
                        if body.cuts(w[0], w[1]) {
                            if kind { &mut t.cut } else { &mut t.marks }.push((kind, i));
                        }
                    }
                }
            }
        }
        for v in [&mut t.cut, &mut t.grazed, &mut t.marks] {
            v.sort_unstable();
            v.dedup();
        }
        t
    }

    /// Whether the sampled curve runs through anything but `allowed` (as
    /// `hits`), stopping at the first.
    pub(super) fn hits_other(
        &self,
        samples: &[Pt],
        marks: bool,
        grow: f64,
        skip: impl Fn(&str, Pt) -> bool,
        allowed: &[Ob],
    ) -> bool {
        let mut other = false;
        self.scan(samples, marks, grow, skip, |ob, _| {
            other = !allowed.contains(&ob);
            !other
        });
        other
    }

    /// `edge` moved: its markers are now these.
    pub(super) fn moved(&mut self, edge: usize, marks: Vec<(&'a str, Body)>) {
        self.marks.replace(edge, marks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::NodeShape;
    use crate::svg::flowchart::edges::EndClip;

    fn body(center: Pt, size: (f64, f64)) -> Body {
        Body::new(&EndClip {
            center,
            size,
            shape: Some(NodeShape::Rect),
        })
        .expect("finite")
    }

    /// The bodies a straight line from `a` to `b` cuts, sampled in `n` steps.
    fn cut(scene: &Scene, a: Pt, b: Pt, n: usize) -> Vec<Ob> {
        let samples: Vec<Pt> = (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
            })
            .collect();
        scene
            .hits(&samples, true, 0.0, |_, _| false)
            .iter()
            .map(|h| h.ob)
            .collect()
    }

    #[test]
    fn the_index_finds_what_a_full_scan_finds() {
        // A dense rank of small nodes, one huge node, one far away and a
        // marker; lines across them in both directions and through the gap.
        let mut nodes: Vec<(&str, Body)> = (0..200)
            .map(|i| ("n", body((f64::from(i) * 100.0, 0.0), (60.0, 40.0))))
            .collect();
        nodes.push(("big", body((10_000.0, 500.0), (1e6, 100.0))));
        nodes.push(("far", body((1e12, 1e12), (60.0, 40.0))));
        let marks = vec![(0, ("m", body((5_000.0, 200.0), (20.0, 20.0))))];
        let scene = Scene::new(nodes, marks);
        assert_eq!(cut(&scene, (-50.0, 0.0), (19_950.0, 0.0), 500).len(), 200);
        assert!(cut(&scene, (-50.0, 100.0), (19_950.0, 100.0), 37).is_empty());
        assert_eq!(
            cut(&scene, (5_000.0, -100.0), (5_000.0, 600.0), 3),
            [(true, 50), (false, 0), (true, 200)]
        );
        assert_eq!(cut(&scene, (1e12, 0.0), (1e12, 2e12), 1), [(true, 201)]);
        // Marks are only looked at on request.
        let samples = [(4_990.0, 200.0), (5_010.0, 200.0)];
        assert!(scene.hits(&samples, false, 0.0, |_, _| false).is_empty());
        assert!(cut(&scene, (f64::NAN, 0.0), (0.0, 0.0), 4).is_empty());
    }
}
