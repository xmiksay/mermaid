//! The other edges' lines and the subgraph frame borders near a detour, and
//! how long a candidate curve runs along them.

use super::super::route::Pt;
use super::body::{dist, dot, unit};
use super::index::Index;
use super::scene::{bbox, segment_box, tail, CHUNK};

/// Closer than this to another line counts as running along it.
const NEAR: f64 = 6.0;
/// A stretch along another line this long is a defect, not a crossing.
const RUN: f64 = 30.0;

/// The other edges' lines and the frame borders near one edge's detour.
pub(super) struct Lines {
    /// Each line: start, unit direction, length, bounding box grown by
    /// `NEAR`.
    segs: Vec<(Pt, Pt, f64, [f64; 4])>,
    index: Index,
    /// Where the lines were gathered: only stretches there are scored.
    region: [f64; 4],
}

fn overlap(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

impl Lines {
    pub(super) fn new(segs: Vec<(Pt, Pt)>, region: [f64; 4]) -> Self {
        let segs: Vec<(Pt, Pt, f64, [f64; 4])> = segs
            .into_iter()
            .filter_map(|(p, q)| {
                let u = unit(p, q)?;
                Some((p, u, dist(p, q), segment_box(p, q, NEAR)))
            })
            .collect();
        let boxes: Vec<(usize, [f64; 4])> = segs.iter().map(|s| s.3).enumerate().collect();
        Self {
            index: Index::new(&boxes),
            segs,
            region,
        }
    }

    /// Whether the sample segment `a`–`b` runs beside line `i`: its middle
    /// within `NEAR` of it, nearly parallel.
    fn beside(&self, i: usize, m: Pt, u: Pt) -> bool {
        let Some(&(p, d, len, bx)) = self.segs.get(i) else {
            return false;
        };
        if !(bx[0] <= m.0 && m.0 <= bx[2] && bx[1] <= m.1 && m.1 <= bx[3]) {
            return false;
        }
        let rel = (m.0 - p.0, m.1 - p.1);
        let t = dot(rel, d).max(0.0).min(len);
        let off = (rel.0 - d.0 * t, rel.1 - d.1 * t);
        off.0 * off.0 + off.1 * off.1 <= NEAR * NEAR && dot(u, d).abs() > 0.9
    }

    /// How long the sampled curve runs along one of these lines (within
    /// `NEAR`, nearly parallel), counting only stretches of at least `RUN`.
    /// Lines are narrowed to the curve's box, then to each run of segments'.
    pub(super) fn along(&self, samples: &[Pt]) -> f64 {
        let (b, r) = (bbox(samples), self.region);
        let b = [
            b[0].max(r[0]),
            b[1].max(r[1]),
            b[2].min(r[2]),
            b[3].min(r[3]),
        ];
        if b[0] > b[2] || b[1] > b[3] {
            return 0.0;
        }
        let mut local = Vec::new();
        if self.index.near(&b, &mut local).is_none() {
            local = (0..self.segs.len()).collect();
        }
        local.retain(|&i| self.segs.get(i).is_some_and(|s| overlap(&s.3, &b)));
        if local.is_empty() {
            return 0.0;
        }
        local.sort_unstable();
        local.dedup();
        let (mut total, mut run) = (0.0, 0.0);
        let mut near: Vec<usize> = Vec::new();
        let runs = samples
            .windows(CHUNK + 1)
            .step_by(CHUNK)
            .chain(tail(samples));
        for chunk in runs {
            let cb = bbox(chunk);
            near.clear();
            near.extend(
                local
                    .iter()
                    .filter(|&&i| self.segs.get(i).is_some_and(|s| overlap(&s.3, &cb))),
            );
            for w in chunk.windows(2) {
                let (a, b) = (w[0], w[1]);
                let len = dist(a, b);
                let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
                if len > 0.0 && near.iter().any(|&i| self.beside(i, m, u)) {
                    run += len;
                } else {
                    if run >= RUN {
                        total += run;
                    }
                    run = 0.0;
                }
            }
        }
        if run >= RUN {
            total += run;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_along_a_line_counts_a_crossing_does_not() {
        let region = [-1000.0, -1000.0, 1000.0, 1000.0];
        let lines = Lines::new(vec![((0.0, 0.0), (0.0, 200.0))], region);
        let walk = |x0: f64, y0: f64, x1: f64, y1: f64| -> Vec<Pt> {
            (0..=40)
                .map(|k| {
                    let t = f64::from(k) / 40.0;
                    (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
                })
                .collect()
        };
        // 3px beside it for 150px: one long stretch.
        let along = lines.along(&walk(3.0, 20.0, 3.0, 170.0));
        assert!((along - 150.0).abs() < 8.0, "{along}");
        // Straight across: nothing.
        assert_eq!(lines.along(&walk(-100.0, 100.0, 100.0, 100.0)), 0.0);
        // Beside it for less than `RUN`: nothing either.
        assert_eq!(lines.along(&walk(3.0, 20.0, 3.0, 40.0)), 0.0);
        // Out of the region it was gathered for: not scored.
        let far = Lines::new(
            vec![((0.0, 0.0), (0.0, 200.0))],
            [500.0, 500.0, 600.0, 600.0],
        );
        assert_eq!(far.along(&walk(3.0, 20.0, 3.0, 170.0)), 0.0);
    }
}
