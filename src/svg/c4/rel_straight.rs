//! Straight grid-mode connectors: centre to centre, clipped to both
//! outlines, parallel ones offset apart, and detoured round anything in the
//! way.

use std::collections::HashMap;

use crate::parse::C4Relation;

use super::grid_route::{blocked, detour, square_off};
use super::outline::{center, clip_route, contains, Rect};
use super::rel_route::{Endpoint, RoutePath};
use super::text::RelLabel;
use super::Placement;

/// Least spacing between parallel straight connectors joining the same two
/// shapes, and how far inside each shape their shifted ends must stay.
const PARALLEL_GAP: f64 = 18.0;
const PARALLEL_INSET: f64 = 8.0;
/// Least distance between two relations' ends on one shape.
const END_GAP: f64 = 12.0;

/// Whether two centre lines run on top of each other: nearly parallel, and
/// each end of one within a few pixels of the other's line.
fn collinear(a: [(f64, f64); 2], b: [(f64, f64); 2]) -> bool {
    const NEAR: f64 = 6.0;
    let dist = |p: (f64, f64), [s, e]: [(f64, f64); 2]| {
        let (dx, dy) = (e.0 - s.0, e.1 - s.1);
        let len = dx.hypot(dy).max(1e-9);
        ((p.0 - s.0) * dy - (p.1 - s.1) * dx).abs() / len
    };
    let (da, db) = (
        (a[1].0 - a[0].0, a[1].1 - a[0].1),
        (b[1].0 - b[0].0, b[1].1 - b[0].1),
    );
    let cos = (da.0 * db.0 + da.1 * db.1).abs() / (da.0.hypot(da.1) * db.0.hypot(db.1)).max(1e-9);
    cos > 0.998 && dist(a[0], b) < NEAR && dist(a[1], b) < NEAR
}

/// Whether `p` lies inside `ep`'s drawn outline, `PARALLEL_INSET` clear of
/// its border.
fn well_inside(p: (f64, f64), ep: Endpoint) -> bool {
    let d = PARALLEL_INSET;
    [(0.0, 0.0), (d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)]
        .iter()
        .all(|(dx, dy)| contains(ep.0, ep.1, (p.0 + dx, p.1 + dy)))
}

/// `p`, or the point nearest it on the way to `ep`'s centre that is
/// `well_inside`.
fn pull_inside(p: (f64, f64), ep: Endpoint) -> (f64, f64) {
    let c = center(ep.1);
    if well_inside(p, ep) || !well_inside(c, ep) {
        return p;
    }
    let (mut a, mut b) = (c, p);
    for _ in 0..32 {
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        if well_inside(m, ep) {
            a = m;
        } else {
            b = m;
        }
    }
    a
}

/// A centre-to-centre line already placed, with its two shapes.
type Drawn = ([(f64, f64); 2], Rect, Rect);
/// A straight connector clipped to its outlines, with its two ends.
type Straight = (Vec<(f64, f64)>, Endpoint, Endpoint);

/// Whether `route`'s end `k` (on `shape`) leaves that shape within 8° of an
/// earlier line `o` that also starts or ends on it.
fn fans_out(route: [(f64, f64); 2], k: usize, shape: Rect, o: &Drawn) -> bool {
    let outward = |r: [(f64, f64); 2], at: usize| {
        let (a, b) = (r[at], r[1 - at]);
        let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
        ((b.0 - a.0) / len, (b.1 - a.1) / len)
    };
    let shared = if o.1 == shape {
        0
    } else if o.2 == shape {
        1
    } else {
        return false;
    };
    let (u, v) = (outward(route, k), outward(o.0, shared));
    u.0 * v.0 + u.1 * v.1 > 0.99
}

/// Straight centre-to-centre connectors, clipped to both outlines; several
/// relations between the same two shapes (either direction) are offset
/// apart in parallel so each label sits on its own line. A connector that
/// would run through a foreign shape or a boundary's header text detours
/// around it instead (`grid_route::detour`, drawn as a spline — the bool).
/// Only relations with `needed[i]` are routed (detours are costly, and in
/// graph mode most relations already have a layout route).
pub(super) fn straight_routes(
    rels: &[C4Relation],
    shapes: &HashMap<&str, Endpoint>,
    placement: &Placement,
    needed: &[bool],
) -> Vec<Option<RoutePath>> {
    let headers: Vec<Rect> = placement
        .boundaries
        .iter()
        .map(|b| b.header_text())
        .collect();
    let pair = |r: &C4Relation| {
        let (a, b) = (r.from.as_str(), r.to.as_str());
        if a <= b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    };
    let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, r) in rels.iter().enumerate() {
        groups.entry(pair(r)).or_default().push(i);
    }
    let mut drawn: Vec<Drawn> = Vec::new();
    let mut ends: Vec<(Rect, (f64, f64))> = Vec::new();
    let straight: Vec<Option<Straight>> = rels
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if !needed.get(i).copied().unwrap_or(false) {
                return None;
            }
            let &start = shapes.get(r.from.as_str())?;
            let &end = shapes.get(r.to.as_str())?;
            let key = pair(r);
            let group = groups.get(&key)?;
            let k = group.iter().position(|&j| j == i).unwrap_or(0);
            // A canonical direction (lower alias → higher) so a reversed pair
            // gets opposite offsets rather than the same one.
            let (lo, hi) = if r.from.as_str() == key.0 {
                (start.1, end.1)
            } else {
                (end.1, start.1)
            };
            let (a, b) = (center(lo), center(hi));
            let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
            let perp = (-(b.1 - a.1) / len, (b.0 - a.0) / len);
            // Far enough apart that each line's label clears the other line,
            // but never so far that a shifted end leaves either shape.
            let label_across = group
                .iter()
                .filter_map(|&j| RelLabel::new(&rels[j], false))
                .map(|l| perp.0.abs() * l.w + perp.1.abs() * l.h)
                .fold(0.0_f64, f64::max);
            let room = [lo, hi]
                .iter()
                .map(|r| {
                    let fit = |half: f64, p: f64| {
                        if p.abs() < 1e-9 {
                            f64::INFINITY
                        } else {
                            half / p.abs()
                        }
                    };
                    let half = fit(r.2 / 2.0, perp.0).min(fit(r.3 / 2.0, perp.1));
                    (half - PARALLEL_INSET).max(0.0)
                })
                .fold(f64::INFINITY, f64::min);
            let spread = (group.len() - 1) as f64;
            let gap = if spread > 0.0 {
                (label_across + PARALLEL_GAP)
                    .min(2.0 * room / spread)
                    .max(PARALLEL_GAP.min(2.0 * room / spread))
            } else {
                0.0
            };
            let off = (k as f64 - spread / 2.0) * gap;
            let shift = |p: (f64, f64)| (p.0 + perp.0 * off, p.1 + perp.1 * off);
            let mut route = [shift(center(start.1)), shift(center(end.1))];
            // A different pair whose line would run on top of this one (a
            // shape and the boundary around its neighbour, say) is nudged
            // aside, as long as both ends stay inside their shapes.
            let inside = |p: (f64, f64), r: Rect| {
                let ep = if r == start.1 { start } else { end };
                well_inside(p, ep)
            };
            let clashes = drawn.iter().filter(|o| collinear(route, o.0)).count();
            if clashes > 0 {
                let d = clashes as f64 * PARALLEL_GAP;
                let (dx, dy) = (perp.0 * d, perp.1 * d);
                let moved = route.map(|p| (p.0 + dx, p.1 + dy));
                if inside(moved[0], start.1) && inside(moved[1], end.1) {
                    route = moved;
                }
            }
            // Lines leaving one shape in nearly the same direction would
            // share their first stretch: slide this one's end along the face.
            for (k, shape) in [(0usize, start.1), (1, end.1)] {
                let fan = drawn
                    .iter()
                    .filter(|o| fans_out(route, k, shape, o))
                    .count();
                if fan > 0 {
                    let d = fan as f64 * PARALLEL_GAP;
                    let moved = (route[k].0 + perp.0 * d, route[k].1 + perp.1 * d);
                    if inside(moved, shape) {
                        route[k] = moved;
                    }
                }
            }
            // The shifts are sized against the bounding box; a rounded
            // outline (a queue's pill, a cylinder's caps) is narrower near
            // its corners, so pull each end back in until it is inside.
            route[0] = pull_inside(route[0], start);
            route[1] = pull_inside(route[1], end);
            // Ends of different relations on one shape keep `END_GAP` apart
            // (two arrowheads meeting at a corner read as one): slide this
            // end across the line until it is clear.
            let clip = |r: [(f64, f64); 2]| clip_route(&r, (start.0, start.1), (end.0, end.1));
            let crowded = |r: [(f64, f64); 2]| {
                let c = clip(r);
                let tips = [(start.1, c[0]), (end.1, c[c.len() - 1])];
                tips.iter().any(|&(shape, p)| {
                    ends.iter().any(|&(o, q): &(Rect, (f64, f64))| {
                        o == shape && (q.0 - p.0).hypot(q.1 - p.1) < END_GAP
                    })
                })
            };
            if crowded(route) {
                let steps = (1..=10).flat_map(|n| [n as f64, -(n as f64)]);
                // One end, the other, or the whole line.
                let slid = [[true, false], [false, true], [true, true]]
                    .into_iter()
                    .find_map(|which| {
                        steps.clone().find_map(|n| {
                            let d = n * END_GAP / 2.0;
                            let mut moved = route;
                            for (k, ep) in [(0usize, start), (1, end)] {
                                if which[k] {
                                    moved[k] = (route[k].0 + perp.0 * d, route[k].1 + perp.1 * d);
                                    if !well_inside(moved[k], ep) {
                                        return None;
                                    }
                                }
                            }
                            (!crowded(moved)).then_some(moved)
                        })
                    });
                if let Some(moved) = slid {
                    route = moved;
                }
            }
            let c = clip(route);
            ends.push((start.1, c[0]));
            ends.push((end.1, c[c.len() - 1]));
            drawn.push((route, start.1, end.1));
            let path = clip_route(&route, (start.0, start.1), (end.0, end.1));
            Some((path, start, end))
        })
        .collect();
    // Then detour the straight lines that run through a shape or a header,
    // keeping off every other relation's line.
    let mut out: Vec<Option<RoutePath>> = straight
        .iter()
        .map(|s| s.as_ref().map(|(p, ..)| (p.clone(), false)))
        .collect();
    for i in 0..straight.len() {
        let Some((path, start, end)) = &straight[i] else {
            continue;
        };
        let obstacles: Vec<Rect> = placement
            .leaves
            .iter()
            .map(|(_, rect)| *rect)
            .filter(|&rect| rect != start.1 && rect != end.1)
            .chain(headers.iter().copied())
            .collect();
        if !blocked(path, &obstacles) {
            continue;
        }
        let others: Vec<Vec<(f64, f64)>> = out
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .filter_map(|(_, o)| o.as_ref().map(|(p, _)| p.clone()))
            .collect();
        let ends = ((start.0, start.1), (end.0, end.1));
        // Ends of every other relation on the two shapes: a detour arriving
        // where another line already arrives would share its last stretch,
        // so the search aims off-centre until its ends are clear.
        let taken: Vec<(f64, f64)> = others.iter().flat_map(|p| [p[0], p[p.len() - 1]]).collect();
        let crowded_at =
            |e: (f64, f64)| taken.iter().any(|q| (q.0 - e.0).hypot(q.1 - e.1) < END_GAP);
        let aims = |r: Rect| {
            let (cx, cy) = center(r);
            let (sx, sy) = (r.2 / 4.0, r.3 / 4.0);
            [(cx - sx, cy), (cx + sx, cy), (cx, cy - sy), (cx, cy + sy)]
        };
        let search = |a: (f64, f64), b: (f64, f64)| {
            let d = detour(a, b, [start.1, end.1], &obstacles, &others)?;
            let clipped = clip_route(&d, ends.0, ends.1);
            let path = square_off(&clipped, (start.1, end.1), &obstacles);
            // The search runs on a grid: make sure the drawn polyline clears
            // every obstacle too.
            (!blocked(&path, &obstacles)).then_some(path)
        };
        // Centre to centre first; if an end lands on another line's end,
        // aim that end off-centre instead.
        let (sc, ec) = (center(start.1), center(end.1));
        let mut found = search(sc, ec);
        if let Some(first) = found.clone() {
            let (head, tail) = (crowded_at(first[0]), crowded_at(first[first.len() - 1]));
            if head || tail {
                let tries: Vec<((f64, f64), (f64, f64))> = if tail {
                    aims(end.1).into_iter().map(|b| (sc, b)).collect()
                } else {
                    aims(start.1).into_iter().map(|a| (a, ec)).collect()
                };
                if let Some(p) = tries.into_iter().find_map(|(a, b)| {
                    search(a, b).filter(|p| !crowded_at(p[0]) && !crowded_at(p[p.len() - 1]))
                }) {
                    found = Some(p);
                }
            }
        }
        if let Some(path) = found {
            out[i] = Some((path, true));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collinear_lines_are_recognised() {
        let a = [(0.0, 0.0), (100.0, 0.0)];
        assert!(collinear(a, [(0.0, 2.0), (200.0, 3.0)]));
        assert!(
            !collinear(a, [(0.0, 20.0), (100.0, 20.0)]),
            "parallel, apart"
        );
        assert!(!collinear(a, [(0.0, 0.0), (100.0, 50.0)]), "diverging");
    }
}
