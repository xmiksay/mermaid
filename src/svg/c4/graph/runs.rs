//! Moving a route's end runs sideways (`ends::spread_ends` decides by how
//! much) without pushing them into shapes or onto other routes' frame
//! crossings.

use std::collections::HashMap;

use super::super::super::label_layout::segment_rect_overlap;
use super::super::outline::{Rect, Span};
use super::super::rel_label::inflate;
use super::{Pt, TRACK_GAP};

/// What a shifted run must keep clear of: every shape's rect (except a
/// relation's own ends, and the members of a boundary it ends on, listed in
/// `skip[i]`), every frame's edges and every boundary's header text.
pub(super) struct Scenery<'a, K> {
    pub shapes: &'a [(K, Rect)],
    pub skip: &'a [Vec<K>],
    pub frames: &'a [Span],
    pub headers: &'a [Rect],
}

/// Where the segment `a`–`b` meets the horizontal line `y` between `x0`
/// and `x1`, if it does.
fn meets(a: Pt, b: Pt, (y, x0, x1): (f64, f64, f64)) -> Option<f64> {
    let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
    if lo > y + 0.5 || hi < y - 0.5 {
        return None;
    }
    let x = if (b.1 - a.1).abs() < 1e-9 {
        a.0
    } else {
        a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0)
    };
    (x >= x0 && x <= x1).then_some(x)
}

/// The closest a shifted run may cross a frame edge to another route's
/// crossing there: any closer and the two stubs through the edge merge.
const EDGE_GAP: f64 = 4.0;
/// Clearance a shifted run keeps from other shapes.
pub(super) const SHIFT_CLEARANCE: f64 = 4.0;
/// Cosine above which two directions count as the same (about 10°).
const PARALLEL: f64 = 0.985;

fn unit(a: Pt, b: Pt) -> Pt {
    let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
    ((b.0 - a.0) / len, (b.1 - a.1) / len)
}
/// Apply each route's end shifts (`(dx, track)` per end).
pub(super) fn apply_shifts<K: Copy + Eq>(
    routes: &mut [Option<Vec<Pt>>],
    shifts: &HashMap<usize, [Option<(f64, usize)>; 2]>,
    ends: &[[Option<(K, f64)>; 2]],
    scenery: &Scenery<K>,
) {
    let Scenery {
        shapes,
        skip,
        frames,
        headers,
    } = scenery;
    // Every frame top and bottom edge, and where each route will cross it
    // once its end runs have moved (assuming they move whole).
    let edges: Vec<(f64, f64, f64)> = frames
        .iter()
        .flat_map(|f| [(f.y0, f.x0, f.x1), (f.y1, f.x0, f.x1)])
        .collect();
    let mut crossings: Vec<(usize, usize, f64, Pt)> = Vec::new();
    for (i, r) in routes.iter().enumerate() {
        let Some(r) = r else { continue };
        let n = r.len();
        let [start, end] = shifts.get(&i).copied().unwrap_or_default();
        let run = |x: f64, it: &mut dyn Iterator<Item = &Pt>| {
            it.take_while(|p| (p.0 - x).abs() < 0.5).count()
        };
        let (lead, tail) = match (r.first(), r.last()) {
            (Some(a), Some(z)) => (run(a.0, &mut r.iter()), run(z.0, &mut r.iter().rev())),
            _ => continue,
        };
        for (k, w) in r.windows(2).enumerate() {
            let dx = if k + 1 < lead {
                start.map_or(0.0, |s| s.0)
            } else if k + tail >= n {
                end.map_or(0.0, |e| e.0)
            } else {
                0.0
            };
            for (e, &edge) in edges.iter().enumerate() {
                if let Some(x) = meets(w[0], w[1], edge) {
                    crossings.push((i, e, x + dx, unit(w[0], w[1])));
                }
            }
        }
    }
    for (&i, &[start, end]) in shifts {
        let own = |k: &K| {
            ends[i].iter().flatten().any(|e| e.0 == *k)
                || skip.get(i).is_some_and(|s| s.contains(k))
        };
        let walls: Vec<Rect> = shapes
            .iter()
            .filter(|(k, _)| !own(k))
            .map(|&(_, r)| inflate(r, SHIFT_CLEARANCE))
            .chain(headers.iter().copied())
            .collect();
        let blocked = |a: Pt, b: Pt| walls.iter().any(|&w| segment_rect_overlap(a, b, w) > 0.0);
        // The crossings were spaced out along each edge while stitching
        // (`route::FrameMarks`); a shift must not bring two that run the
        // same way through the edge together.
        let on_frame = |a: Pt, b: Pt| {
            let u = unit(a, b);
            (0..edges.len()).any(|e| {
                meets(a, b, edges[e]).is_some_and(|x| {
                    crossings.iter().any(|&(j, f, cx, v)| {
                        j != i
                            && f == e
                            && (cx - x).abs() < EDGE_GAP
                            && (u.0 * v.0 + u.1 * v.1).abs() > PARALLEL
                    })
                })
            })
        };
        if let Some(r) = routes[i].as_mut() {
            shift_runs(r, start, end, &blocked, &on_frame);
        }
    }
}

/// Move the straight run at each end of `r` by that end's sideways shift,
/// and push the run's far point `track × TRACK_GAP` further out (capped well
/// short of the next waypoint). A route that is one straight run throughout
/// moves whole when only one end shifts, and is split between its ends when
/// both do.
///
/// A run may be long — down a column of dummies through several layers,
/// across frame edges — and the column beside it is not checked by the
/// layout: only the part of the run that stays clear of every shape
/// (`blocked`) and short of every frame edge (`on_frame`, where the route's
/// crossing was already spaced out from its neighbours') moves, and the
/// route jogs back to its own column after it.
fn shift_runs(
    r: &mut [Pt],
    start: Option<(f64, usize)>,
    end: Option<(f64, usize)>,
    blocked: &dyn Fn(Pt, Pt) -> bool,
    on_frame: &dyn Fn(Pt, Pt) -> bool,
) {
    let n = r.len();
    if n == 0 {
        return;
    }
    let (start_dx, end_dx) = (start.map(|s| s.0), end.map(|e| e.0));
    let same = |a: f64, b: f64| (a - b).abs() < 0.5;
    let (x0, x1) = (r[0].0, r[n - 1].0);
    if r.iter().all(|p| same(p.0, x0)) {
        let half = n.div_ceil(2);
        let (a, b) = match (start_dx, end_dx) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) => (a, a),
            (None, Some(b)) => (b, b),
            (None, None) => return,
        };
        let moved: Vec<Pt> = (0..n)
            .map(|j| (r[j].0 + if j < half { a } else { b }, r[j].1))
            .collect();
        if !moved.windows(2).any(|w| blocked(w[0], w[1])) {
            r.copy_from_slice(&moved);
        }
        return;
    }
    let lead = r.iter().take_while(|p| same(p.0, x0)).count();
    let tail = r.iter().rev().take_while(|p| same(p.0, x1)).count();
    if let Some((dx, track)) = start {
        let m = clear_prefix(r, 0..lead, dx, (blocked, on_frame));
        r[..m].iter_mut().for_each(|p| p.0 += dx);
        if m == lead {
            deepen(r, lead - 1, lead, track);
        }
    }
    if let Some((dx, track)) = end {
        let m = clear_prefix(r, (n - tail..n).rev(), dx, (blocked, on_frame));
        r[n - m..].iter_mut().for_each(|p| p.0 += dx);
        if m == tail {
            deepen(r, n - tail, n - tail - 1, track);
        }
    }
}

/// How many points of the run `idx` (walked from the route's end) can move
/// by `dx` with the moved run clear of every shape and frame edge, and the
/// jog back to the unmoved remainder clear of every shape.
type Check<'c> = &'c dyn Fn(Pt, Pt) -> bool;
fn clear_prefix(
    r: &[Pt],
    idx: impl Iterator<Item = usize>,
    dx: f64,
    (blocked, on_frame): (Check, Check),
) -> usize {
    let idx: Vec<usize> = idx.collect();
    let moved = |j: usize| (r[j].0 + dx, r[j].1);
    let next = |k: usize| {
        let j = idx[k];
        if idx.len() > 1 && idx[1] < idx[0] {
            j.checked_sub(1)
        } else {
            (j + 1 < r.len()).then_some(j + 1)
        }
    };
    let mut m = 1;
    while m < idx.len() && {
        let (a, b) = (moved(idx[m - 1]), moved(idx[m]));
        !blocked(a, b) && !on_frame(a, b)
    } {
        m += 1;
    }
    // Shorten until the jog back onto the unmoved route is clear too.
    while m > 0 && m < idx.len() && next(m - 1).is_some_and(|j| blocked(moved(idx[m - 1]), r[j])) {
        m -= 1;
    }
    m
}

/// Push `r[far]` (a run's far end) away from its shape by `track` tracks,
/// towards `r[next]` but never past 60% of the way there.
fn deepen(r: &mut [Pt], far: usize, next: usize, track: usize) {
    if track == 0 || next >= r.len() {
        return;
    }
    let room = r[next].1 - r[far].1;
    let push = (track as f64 * TRACK_GAP).min(0.6 * room.abs());
    r[far].1 += room.signum() * push;
}
