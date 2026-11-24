//! Keeping two routes from merging along a leg: long shallow legs between
//! layer bands can meet another route's at a few degrees and run on top of
//! it for tens of pixels. The later route bows away there.

use std::collections::HashMap;

use super::super::super::label_layout::segment_rect_overlap;
use super::super::outline::Rect;
use super::Pt;

/// Two legs within this many degrees (as a cosine) count as running the
/// same way, and within `NEAR` px of each other for over `RUN` px as merged.
const SAME_WAY: f64 = 0.995;
const NEAR: f64 = 3.0;
const RUN: f64 = 4.0;
/// How far the bow pushes the later leg off the earlier one.
const BOW: f64 = 10.0;
/// A merged stretch this close to a leg's end is fixed at that end.
const END_REACH: f64 = 30.0;
/// Repair passes: a bow can bring a route near a third one.
const PASSES: usize = 3;

fn unit(a: Pt, b: Pt) -> Option<Pt> {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    (len > 1e-9).then(|| ((b.0 - a.0) / len, (b.1 - a.1) / len))
}

fn seg_distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Every route's legs bucketed in a uniform grid.
struct SegIndex {
    segs: Vec<(usize, Pt, Pt)>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

const CELL: f64 = 64.0;

fn cells_of(a: Pt, b: Pt) -> impl Iterator<Item = (i64, i64)> {
    let cell = |v: f64| (v / CELL).floor() as i64;
    let (x0, x1) = (cell(a.0.min(b.0) - NEAR), cell(a.0.max(b.0) + NEAR));
    let (y0, y1) = (cell(a.1.min(b.1) - NEAR), cell(a.1.max(b.1) + NEAR));
    (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
}

impl SegIndex {
    fn new(routes: &[Option<Vec<Pt>>]) -> Self {
        let mut index = Self {
            segs: Vec::new(),
            cells: HashMap::new(),
        };
        for (i, r) in routes.iter().enumerate() {
            for w in r.iter().flat_map(|r| r.windows(2)) {
                let n = index.segs.len();
                index.segs.push((i, w[0], w[1]));
                for c in cells_of(w[0], w[1]) {
                    index.cells.entry(c).or_default().push(n);
                }
            }
        }
        index
    }

    /// Legs (route, ends) whose cells meet those of `a`–`b`.
    fn near(&self, a: Pt, b: Pt) -> impl Iterator<Item = (usize, Pt, Pt)> + '_ {
        let mut found: Vec<usize> = cells_of(a, b)
            .filter_map(|c| self.cells.get(&c))
            .flatten()
            .copied()
            .collect();
        found.sort_unstable();
        found.dedup();
        found.into_iter().map(|n| self.segs[n])
    }
}

/// Whether the segments `a`–`b` and `c`–`d` never come within `NEAR`:
/// neither crosses the other and every end is that far from the other.
fn apart(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let cross = |o: Pt, p: Pt, q: Pt| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    let intersect = cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0;
    !intersect
        && seg_distance(a, c, d) >= NEAR
        && seg_distance(b, c, d) >= NEAR
        && seg_distance(c, a, b) >= NEAR
        && seg_distance(d, a, b) >= NEAR
}

/// Where along leg `(a, b)` it runs merged with `(c, d)`: the middle of the
/// longest stretch within `NEAR`, if longer than `RUN`.
fn merged(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<Pt> {
    let (u, v) = (unit(a, b)?, unit(c, d)?);
    if (u.0 * v.0 + u.1 * v.1).abs() < SAME_WAY || apart(a, b, c, d) {
        return None;
    }
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let steps = len.ceil() as usize;
    let (mut run, mut best, mut at) = (0.0, 0.0, None);
    let mut start = 0;
    for k in 0..=steps {
        let t = k as f64 / steps.max(1) as f64;
        let p = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        if seg_distance(p, c, d) < NEAR {
            if run == 0.0 {
                start = k;
            }
            run += len / steps.max(1) as f64;
            if run > best {
                best = run;
                let mid = (start + k) as f64 / 2.0 / steps.max(1) as f64;
                at = Some((a.0 + mid * (b.0 - a.0), a.1 + mid * (b.1 - a.1)));
            }
        } else {
            run = 0.0;
        }
    }
    at.filter(|_| best > RUN)
}

/// Bow every later leg that runs merged with an earlier route's leg away
/// from it, by a doubled control point `BOW` px off the merged stretch —
/// unless that would put it into one of `shapes`.
pub(super) fn bow_apart(routes: &mut [Option<Vec<Pt>>], shapes: &[Rect]) {
    let blocked = |a: Pt, b: Pt| shapes.iter().any(|&r| segment_rect_overlap(a, b, r) > 0.0);
    for _ in 0..PASSES {
        let index = SegIndex::new(routes);
        // Routes bowed this pass: the index still has their old legs.
        let mut dirty: Vec<usize> = Vec::new();
        let mut changed = false;
        for j in 0..routes.len() {
            let Some(rj) = &routes[j] else { continue };
            let hit = rj.windows(2).enumerate().find_map(|(k, w)| {
                let fresh = dirty.iter().filter(|&&i| i < j).flat_map(|&i| {
                    routes[i]
                        .iter()
                        .flat_map(|r| r.windows(2))
                        .map(move |v| (i, v[0], v[1]))
                        .filter(|&(_, c, d)| {
                            c.0.min(d.0) <= w[0].0.max(w[1].0) + NEAR
                                && w[0].0.min(w[1].0) <= c.0.max(d.0) + NEAR
                                && c.1.min(d.1) <= w[0].1.max(w[1].1) + NEAR
                                && w[0].1.min(w[1].1) <= c.1.max(d.1) + NEAR
                        })
                });
                index
                    .near(w[0], w[1])
                    .filter(|(i, ..)| !dirty.contains(i))
                    .chain(fresh)
                    .find_map(|(i, c, d)| {
                        (i < j)
                            .then(|| merged(w[0], w[1], c, d))
                            .flatten()
                            .map(|at| (k, at, c, d))
                    })
            });
            let Some((k, at, c, d)) = hit else { continue };
            let Some(u) = unit(c, d) else { continue };
            let normal = (-u.1, u.0);
            // Away from the other leg: the side `at` already leans to.
            let side = (at.0 - c.0) * normal.0 + (at.1 - c.1) * normal.1;
            let signs = if side >= 0.0 {
                [1.0, -1.0]
            } else {
                [-1.0, 1.0]
            };
            let (a, b) = (rj[k], rj[k + 1]);
            // Merged by a leg's end, which can't move: come into (or
            // leave) that end straight along the flow axis instead.
            let steep = [a, b]
                .into_iter()
                .find(|e| (e.0 - at.0).hypot(e.1 - at.1) < END_REACH)
                .map(|e| {
                    let other = if e == a { b } else { a };
                    let dy = other.1 - e.1;
                    (e.0, e.1 + dy.signum() * (dy.abs() / 3.0).min(END_REACH))
                });
            let bows = signs
                .iter()
                .map(|s| (at.0 + s * BOW * normal.0, at.1 + s * BOW * normal.1));
            let Some(m) = steep
                .into_iter()
                .chain(bows)
                .find(|&m| !blocked(a, m) && !blocked(m, b))
            else {
                continue;
            };
            if let Some(r) = routes[j].as_mut() {
                r.splice(k + 1..k + 1, [m, m]);
                changed = true;
                dirty.push(j);
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leg_merging_with_another_bows_away() {
        let mut routes = vec![
            Some(vec![(0.0, 0.0), (400.0, 40.0)]),
            Some(vec![(0.0, 2.0), (400.0, 41.0)]),
        ];
        bow_apart(&mut routes, &[]);
        let r = routes[1].as_ref().expect("route");
        assert!(r.len() > 2, "{r:?}");
        assert!(seg_distance(r[1], (0.0, 0.0), (400.0, 40.0)) >= BOW - 1.0);
        // Legs at a clear angle are left alone.
        let mut crossing = vec![
            Some(vec![(0.0, 0.0), (400.0, 0.0)]),
            Some(vec![(0.0, -100.0), (400.0, 100.0)]),
        ];
        bow_apart(&mut crossing, &[]);
        assert_eq!(crossing[1].as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn a_bow_never_enters_a_shape() {
        let mut routes = vec![
            Some(vec![(0.0, 0.0), (400.0, 0.0)]),
            Some(vec![(0.0, 1.0), (400.0, 1.0)]),
        ];
        let walls = [(150.0, -40.0, 100.0, 35.0), (150.0, 5.0, 100.0, 35.0)];
        bow_apart(&mut routes, &walls);
        assert_eq!(routes[1].as_ref().map(Vec::len), Some(2));
    }
}
