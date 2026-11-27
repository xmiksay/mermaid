//! The obstacles of one diagram, indexed so a curve only tests the bodies
//! near it, and which of them a sampled curve runs through.

use std::collections::BTreeMap;

use super::super::route::Pt;
use super::body::Body;

/// One body a curve runs through: its index in the scene and the first and
/// last sample segments inside it.
pub(super) struct Hit {
    pub(super) ob: usize,
    pub(super) first: usize,
    pub(super) last: usize,
}

/// Segments looked up together.
const CHUNK: usize = 16;

/// The last, shorter run of segments `windows(CHUNK + 1).step_by(CHUNK)`
/// leaves out (all of them when there are fewer than `CHUNK`).
fn tail(samples: &[Pt]) -> Option<&[Pt]> {
    let segs = samples.len().checked_sub(1)?;
    let done = segs / CHUNK * CHUNK;
    (done < segs).then(|| samples.get(done..)).flatten()
}

/// Smallest band.
const MIN_BAND: f64 = 8.0;
/// Most bands across one axis; a larger scene gets wider bands.
const MAX_BANDS: f64 = 65_536.0;
/// Most bands one box may cross before it skips the index.
const MAX_CROSS: usize = 64;

/// One band: `(lo, hi, body)` on the other axis, sorted by `lo`, and the
/// band's longest `hi - lo`.
type Band = (Vec<(f64, f64, usize)>, f64);

/// Bodies in bands across one axis (`axis`: 0 bands by x, 1 by y), each band
/// sorted along the other axis, so a lookup is a binary search per band.
#[derive(Default)]
struct Bands {
    axis: usize,
    origin: f64,
    size: f64,
    bands: Vec<Band>,
    /// Bodies crossing too many bands, near every box.
    wide: Vec<usize>,
}

impl Bands {
    fn new(items: &[(usize, [f64; 4])], axis: usize) -> Self {
        let (lo, hi) = items
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |m, (_, b)| {
                (m.0.min(b[axis]), m.1.max(b[axis + 2]))
            });
        let len = hi - lo;
        let mut out = Self {
            axis,
            ..Self::default()
        };
        if !len.is_finite() {
            out.wide = items.iter().map(|&(i, _)| i).collect();
            return out;
        }
        // Bands about one body thick: a segment then meets few bodies it
        // does not overlap, even beside a dense rank.
        let mut sides: Vec<f64> = items.iter().map(|(_, b)| b[axis + 2] - b[axis]).collect();
        sides.sort_by(f64::total_cmp);
        let median = sides.get(sides.len() / 2).copied().unwrap_or(MIN_BAND);
        (out.origin, out.size) = (lo, median.max(MIN_BAND).max(len / MAX_BANDS));
        out.bands = vec![(Vec::new(), 0.0); (len / out.size) as usize + 1];
        let other = 1 - axis;
        for &(i, b) in items {
            match out.cross(b[axis], b[axis + 2]) {
                Some((first, last)) => {
                    for (band, reach) in out.bands.iter_mut().take(last + 1).skip(first) {
                        band.push((b[other], b[other + 2], i));
                        *reach = reach.max(b[other + 2] - b[other]);
                    }
                }
                None => out.wide.push(i),
            }
        }
        for (band, _) in &mut out.bands {
            band.sort_by(|p, q| p.0.total_cmp(&q.0));
        }
        out
    }

    /// The bands `lo..=hi` crosses, clipped to the index; `None` past
    /// `MAX_CROSS` bands or for non-finite input.
    fn cross(&self, lo: f64, hi: f64) -> Option<(usize, usize)> {
        let at = |v: f64| ((v - self.origin) / self.size).floor();
        let (a, b) = (at(lo).max(0.0), at(hi).min(self.bands.len() as f64 - 1.0));
        if !(a.is_finite() && b.is_finite()) || b - a >= MAX_CROSS as f64 {
            return None;
        }
        Some((a as usize, (b.max(a)) as usize))
    }

    /// How many bands the box `b` crosses, if it can be looked up.
    fn cost(&self, b: &[f64; 4]) -> Option<usize> {
        let (lo, hi) = (b[self.axis], b[self.axis + 2]);
        if hi < self.origin || lo > self.origin + self.size * self.bands.len() as f64 {
            return Some(0);
        }
        self.cross(lo, hi).map(|(a, z)| z - a + 1)
    }

    /// Bodies that may overlap the box `b`, possibly repeated.
    fn near(&self, b: &[f64; 4], out: &mut Vec<usize>) {
        out.extend(&self.wide);
        let other = 1 - self.axis;
        let (qlo, qhi) = (b[other], b[other + 2]);
        let (lo, hi) = (b[self.axis], b[self.axis + 2]);
        if hi < self.origin || lo > self.origin + self.size * self.bands.len() as f64 {
            return;
        }
        let Some((first, last)) = self.cross(lo, hi) else {
            return;
        };
        for (band, reach) in self.bands.iter().take(last + 1).skip(first) {
            let start = band.partition_point(|e| e.0 < qlo - reach);
            let end = band.partition_point(|e| e.0 <= qhi);
            out.extend(band.get(start..end).into_iter().flatten().map(|e| e.2));
        }
    }
}

/// One kind of body, indexed in rows and in columns; a box is looked up in
/// whichever it crosses fewer of.
struct Index {
    count: usize,
    rows: Bands,
    cols: Bands,
}

impl Index {
    fn new(items: &[(usize, [f64; 4])]) -> Self {
        Self {
            count: items.len(),
            rows: Bands::new(items, 1),
            cols: Bands::new(items, 0),
        }
    }

    /// Bodies that may overlap the box `b`, possibly repeated; `None` when
    /// the box is too large to look up.
    fn near(&self, b: &[f64; 4], out: &mut Vec<usize>) -> Option<()> {
        match (self.rows.cost(b), self.cols.cost(b)) {
            (Some(r), Some(c)) if c < r => self.cols.near(b, out),
            (Some(_), _) => self.rows.near(b, out),
            (None, Some(_)) => self.cols.near(b, out),
            (None, None) => return (self.count == 0).then_some(()),
        }
        Some(())
    }
}

/// Every obstacle — nodes first, then markers — with the id of the node it
/// belongs to, and a grid over each kind, so a curve only tests the bodies
/// near each of its segments.
pub(super) struct Scene<'a> {
    pub(super) bodies: Vec<Body>,
    pub(super) ids: Vec<&'a str>,
    /// Bodies below this index are nodes.
    pub(super) nodes: usize,
    index: [Index; 2],
}

impl<'a> Scene<'a> {
    pub(super) fn new(nodes: Vec<(&'a str, Body)>, marks: Vec<(&'a str, Body)>) -> Self {
        let count = nodes.len();
        let (ids, bodies): (Vec<&str>, Vec<Body>) = nodes.into_iter().chain(marks).unzip();
        let boxes: Vec<(usize, [f64; 4])> = bodies.iter().map(|b| b.bbox).enumerate().collect();
        let (n, m) = boxes.split_at(count.min(boxes.len()));
        let index = [Index::new(n), Index::new(m)];
        Self {
            bodies,
            ids,
            nodes: count,
            index,
        }
    }

    /// The nodes — and with `marks` the markers — the sampled curve runs
    /// through, but those `skip` names, in the order the curve reaches them.
    pub(super) fn hits(
        &self,
        samples: &[Pt],
        marks: bool,
        skip: impl Fn(usize) -> bool,
    ) -> Vec<Hit> {
        let index = if marks {
            &self.index[..]
        } else {
            &self.index[..1]
        };
        let mut found: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
        let mut near = Vec::new();
        // Look bodies up once per run of segments, then test each segment
        // only against those overlapping the run.
        for (c, run) in samples
            .windows(CHUNK + 1)
            .step_by(CHUNK)
            .chain(tail(samples))
            .enumerate()
        {
            let inf = f64::INFINITY;
            let bbox = run.iter().fold([inf, inf, -inf, -inf], |m, p| {
                [m[0].min(p.0), m[1].min(p.1), m[2].max(p.0), m[3].max(p.1)]
            });
            near.clear();
            for g in index {
                if g.near(&bbox, &mut near).is_none() {
                    let all = if marks { self.bodies.len() } else { self.nodes };
                    near.clear();
                    near.extend(0..all);
                    break;
                }
            }
            near.retain(|&ob| self.bodies.get(ob).is_some_and(|body| body.overlaps(&bbox)));
            near.sort_unstable();
            near.dedup();
            for (k, w) in run.windows(2).enumerate() {
                let i = c * CHUNK + k;
                for &ob in &near {
                    if self
                        .bodies
                        .get(ob)
                        .is_some_and(|body| body.cuts(w[0], w[1]))
                        && !skip(ob)
                    {
                        found.entry(ob).or_insert((i, i)).1 = i;
                    }
                }
            }
        }
        let mut out: Vec<Hit> = found
            .into_iter()
            .map(|(ob, (first, last))| Hit { ob, first, last })
            .collect();
        out.sort_by_key(|h| (h.first, h.ob));
        out
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
    fn cut(scene: &Scene, a: Pt, b: Pt, n: usize) -> Vec<usize> {
        let samples: Vec<Pt> = (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
            })
            .collect();
        scene
            .hits(&samples, true, |_| false)
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
        let marks = vec![("m", body((5_000.0, 200.0), (20.0, 20.0)))];
        let scene = Scene::new(nodes, marks);
        assert_eq!(cut(&scene, (-50.0, 0.0), (19_950.0, 0.0), 500).len(), 200);
        assert!(cut(&scene, (-50.0, 100.0), (19_950.0, 100.0), 37).is_empty());
        assert_eq!(
            cut(&scene, (5_000.0, -100.0), (5_000.0, 600.0), 3),
            [50, 202, 200]
        );
        assert_eq!(cut(&scene, (1e12, 0.0), (1e12, 2e12), 1), [201]);
        // Marks are only looked at on request.
        let samples = [(4_990.0, 200.0), (5_010.0, 200.0)];
        assert!(scene.hits(&samples, false, |_| false).is_empty());
        assert!(cut(&scene, (f64::NAN, 0.0), (0.0, 0.0), 4).is_empty());
    }
}
