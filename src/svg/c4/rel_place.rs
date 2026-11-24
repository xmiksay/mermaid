//! Finding a spot for a relation's label block. Labels never overlap a
//! shape, a header, an arrowhead or another label: those are hard
//! constraints every stage keeps. First choice is on the label's own line
//! (sidestepped at most `MAX_SIDESTEP`); failing that, the nearest clear
//! spot beside the line, joined to it by a leader; failing that, a spot past
//! everything already drawn, also with a leader.

use super::super::label_layout::rect_at;
use super::arc_path::ArcPath;
use super::outline::Rect;
use super::rel_label::{inflate, Obstacles, Owner};

type Pt = (f64, f64);

/// A few pixels of breathing room a label's background must keep clear of
/// another relation's routed line — `segment_rect_overlap` alone only
/// catches a literal intersection, which still lets a line pass close enough
/// to a label to read as touching (#399).
const LINE_CLEARANCE: f64 = 4.0;

/// Candidate positions per sidestep, spread along the line by arc length.
const STEPS: usize = 80;
/// Sidesteps perpendicular to the line, in `SIDE_STEP`s, at most
/// `MAX_SIDESTEP` — and never so far that the label stops covering its own
/// line, so it always reads as that line's label.
const SIDE_STEP: f64 = 6.0;
const MAX_SIDESTEP: f64 = 24.0;
/// Cost per pixel of foreign line under the label, and per pixel of
/// sidestep: an opaque label hiding a line is worse than a label slightly
/// off-centre on its own line.
const CROSSING_COST: f64 = 10.0;
/// Cost per square pixel of a firm obstacle (an arrowhead zone, a frame
/// border) under the label.
const FIRM_COST: f64 = 3.0;
const OFFSET_COST: f64 = 0.5;
/// Off-line search: gaps between the line and the label's near edge, in
/// `FAR_STEP`s up to `MAX_FAR`, at `FAR_STEPS` spots along the line.
const FAR_STEP: f64 = 8.0;
const MAX_FAR: f64 = 320.0;
const FAR_STEPS: usize = 32;
/// How far off its line a label may go to avoid covering another line.
const NEAR_LEADER: f64 = 40.0;
/// Cost per pixel of leader length.
const LEADER_COST: f64 = 1.0;

/// Where a label goes: its centre and, when it sits off its line, a leader
/// from its edge to the line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Spot {
    pub centre: Pt,
    pub leader: Option<(Pt, Pt)>,
}

/// Pick the spot for a `size` label along `path` (see the module docs).
///
/// On the line, candidates run outward from the arc-length midpoint, first
/// on the line itself, then sidestepped perpendicular to it — but only as
/// far as the label still covers its own line, and never closer to a
/// foreign line than to its own. Of the candidates clear of every hard
/// rect, the first also clear of foreign lines (those `foreign` accepts)
/// and firm rects wins, otherwise the cheapest by
/// `FIRM_COST`/`CROSSING_COST`/`OFFSET_COST`.
pub(super) fn place_label(
    path: &[Pt],
    size: Pt,
    obstacles: &Obstacles,
    foreign: &dyn Fn(Owner) -> bool,
) -> Spot {
    let Some(&first) = path.first() else {
        return Spot {
            centre: (0.0, 0.0),
            leader: None,
        };
    };
    let arc = ArcPath::new(path);
    if path.len() < 2 || arc.total() < 1e-9 {
        return beyond(first, size, obstacles);
    }
    let on = on_line(&arc, size, obstacles, foreign);
    if let Some((c, true)) = on {
        return Spot {
            centre: c,
            leader: None,
        };
    }
    // Every spot on the line covers another line or a firm obstacle: a spot
    // just beside it, clear of both, reads better with a short leader.
    let reach = if on.is_some() { NEAR_LEADER } else { MAX_FAR };
    let off = off_line(&arc, size, obstacles, foreign, reach);
    if let Some((spot, true)) = off {
        return spot;
    }
    if let Some((c, _)) = on {
        return Spot {
            centre: c,
            leader: None,
        };
    }
    off.map(|(spot, _)| spot).unwrap_or_else(|| {
        let (mid, _) = arc.at(arc.total() / 2.0);
        beyond(mid, size, obstacles)
    })
}

/// The best spot on the line clear of every hard rect, and whether it is
/// also clear of foreign lines and firm rects.
fn on_line(
    arc: &ArcPath,
    size: Pt,
    obstacles: &Obstacles,
    foreign: &dyn Fn(Owner) -> bool,
) -> Option<(Pt, bool)> {
    let total = arc.total();
    let mut best: Option<(f64, Pt)> = None;
    let sides = (MAX_SIDESTEP / SIDE_STEP) as usize;
    for side in 0..=2 * sides {
        let off = alternate(side) * SIDE_STEP;
        for i in 0..=STEPS {
            let along = total / 2.0 + (alternate(i) / STEPS as f64) * total;
            let (p, (ux, uy)) = arc.at(along);
            // Half the label's extent across the line: sidestep no further.
            let across = uy.abs() * size.0 / 2.0 + ux.abs() * size.1 / 2.0;
            if off.abs() > (across - 2.0).max(0.0) {
                continue;
            }
            let c = (p.0 - uy * off, p.1 + ux * off);
            let rect = rect_at(c, size);
            if obstacles.covered(rect) > 0.0 {
                continue;
            }
            if off != 0.0 && obstacles.foreign_within(c, off.abs(), foreign) {
                continue;
            }
            let firm = obstacles.firmly_covered(rect);
            let crossed = obstacles.crossed(inflate(rect, LINE_CLEARANCE), foreign);
            if crossed <= 0.0 && firm <= 0.0 {
                return Some((c, true));
            }
            let cost = firm * FIRM_COST + crossed * CROSSING_COST + off.abs() * OFFSET_COST;
            if best.is_none_or(|(b, _)| cost < b) {
                best = Some((cost, c));
            }
        }
    }
    best.map(|(_, c)| (c, false))
}

/// The nearest clear spot beside the line (at most `max_gap` off it), by
/// leader length plus the same firm/crossing costs; the first gap with a
/// spot clear of everything wins. Also whether it is clear of everything.
fn off_line(
    arc: &ArcPath,
    size: Pt,
    obstacles: &Obstacles,
    foreign: &dyn Fn(Owner) -> bool,
    max_gap: f64,
) -> Option<(Spot, bool)> {
    let total = arc.total();
    let mut best: Option<(f64, Spot)> = None;
    let mut gap = FAR_STEP;
    while gap <= max_gap {
        for i in 0..=FAR_STEPS {
            let along = total * i as f64 / FAR_STEPS as f64;
            let (p, (ux, uy)) = arc.at(along);
            let across = uy.abs() * size.0 / 2.0 + ux.abs() * size.1 / 2.0;
            for sign in [1.0, -1.0] {
                let off = sign * (across + gap);
                let c = (p.0 - uy * off, p.1 + ux * off);
                let rect = rect_at(c, size);
                if obstacles.covered(rect) > 0.0 {
                    continue;
                }
                let leader = (nearest_on(rect, p), p);
                let firm = obstacles.firmly_covered(rect);
                let crossed = obstacles.crossed(inflate(rect, LINE_CLEARANCE), foreign);
                let cost = gap * LEADER_COST + firm * FIRM_COST + crossed * CROSSING_COST;
                let spot = Spot {
                    centre: c,
                    leader: Some(leader),
                };
                if best.is_none_or(|(b, _)| cost < b) {
                    best = Some((cost, spot));
                }
            }
        }
        if let Some((_, spot)) = best.filter(|(cost, _)| *cost <= gap * LEADER_COST) {
            return Some((spot, true));
        }
        gap += FAR_STEP;
    }
    best.map(|(_, s)| (s, false))
}

/// Below everything placed so far: always clear, since no obstacle reaches
/// past the obstacles' extent.
fn beyond(anchor: Pt, size: Pt, obstacles: &Obstacles) -> Spot {
    let bottom = obstacles.extent().map_or(anchor.1, |e| e.3);
    let centre = (anchor.0, bottom + LINE_CLEARANCE + size.1 / 2.0 + 1.0);
    let rect = rect_at(centre, size);
    Spot {
        centre,
        leader: Some((nearest_on(rect, anchor), anchor)),
    }
}

/// The point of `r`'s outline nearest `p`.
fn nearest_on(r: Rect, p: Pt) -> Pt {
    (p.0.clamp(r.0, r.0 + r.2), p.1.clamp(r.1, r.1 + r.3))
}

/// `0, +1, -1, +2, -2, …` for `k = 0, 1, 2, 3, 4, …`.
fn alternate(k: usize) -> f64 {
    let step = k.div_ceil(2) as f64;
    if k % 2 == 1 {
        step
    } else {
        -step
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::label_layout::{rect_overlap_area, segment_rect_overlap};
    use super::super::rel_label::Segment;
    use super::*;

    const SIZE: (f64, f64) = (20.0, 14.0);

    fn index(rects: &[Rect], lines: &[Segment]) -> Obstacles {
        let mut o = Obstacles::default();
        for &r in rects {
            o.add_rect(r);
        }
        for &s in lines {
            o.add_segment(s, None);
        }
        o
    }

    fn place(
        path: &[(f64, f64)],
        size: (f64, f64),
        rects: &[Rect],
        lines: &[Segment],
    ) -> (f64, f64) {
        place_label(path, size, &index(rects, lines), &|_| true).centre
    }

    #[test]
    fn unobstructed_label_sits_at_the_midpoint() {
        assert_eq!(
            place(&[(0.0, 0.0), (100.0, 0.0)], SIZE, &[], &[]),
            (50.0, 0.0)
        );
    }

    #[test]
    fn dodges_a_shape_straddling_the_midpoint() {
        // #366: a third shape sits on the line, covering its midpoint.
        let obstacle = (30.0, -10.0, 40.0, 20.0);
        let (mx, my) = place(&[(0.0, 0.0), (100.0, 0.0)], SIZE, &[obstacle], &[]);
        assert_eq!(rect_overlap_area(rect_at((mx, my), SIZE), obstacle), 0.0);
        assert_eq!(my, 0.0, "label should stay on the relation line");
    }

    #[test]
    fn walks_along_a_bent_polyline() {
        // The midpoint of this L-shaped route is its corner, which is blocked.
        let path = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let obstacle = (60.0, -20.0, 60.0, 60.0);
        let (mx, my) = place(&path, SIZE, &[obstacle], &[]);
        assert_eq!(rect_overlap_area(rect_at((mx, my), SIZE), obstacle), 0.0);
    }

    #[test]
    fn steps_off_the_line_with_a_leader_when_it_must() {
        // Shapes hug the line on both sides along its whole length: the
        // label goes beside them, joined to the line by a leader.
        let rects = [
            (-10.0, -30.0, 120.0, 26.0),
            (-10.0, 4.0, 120.0, 26.0),
            (40.0, -4.0, 20.0, 8.0),
        ];
        let spot = place_label(
            &[(0.0, 0.0), (100.0, 0.0)],
            SIZE,
            &index(&rects, &[]),
            &|_| true,
        );
        let r = rect_at(spot.centre, SIZE);
        assert!(
            rects.iter().all(|&o| rect_overlap_area(r, o) == 0.0),
            "{spot:?}"
        );
        let (on_label, on_line) = spot.leader.expect("a leader");
        assert_eq!(on_line.1, 0.0);
        assert!(on_label.0 >= r.0 && on_label.0 <= r.0 + r.2);
    }

    #[test]
    fn never_overlaps_even_when_boxed_in() {
        // Everything near the line is taken: the label goes past it all.
        let obstacle = (-1000.0, -1000.0, 2100.0, 2000.0);
        let spot = place_label(
            &[(0.0, 0.0), (100.0, 0.0)],
            SIZE,
            &index(&[obstacle], &[]),
            &|_| true,
        );
        assert_eq!(rect_overlap_area(rect_at(spot.centre, SIZE), obstacle), 0.0);
        assert!(spot.leader.is_some());
    }

    #[test]
    fn dodges_a_crossing_line() {
        // #382: an unrelated relation crosses this one at its midpoint.
        let crossing = ((50.0, -50.0), (50.0, 50.0));
        let (mx, my) = place(&[(0.0, 0.0), (100.0, 0.0)], SIZE, &[], &[crossing]);
        let r = rect_at((mx, my), SIZE);
        assert_eq!(segment_rect_overlap(crossing.0, crossing.1, r), 0.0);
    }

    #[test]
    fn a_label_crowded_by_lines_steps_aside_on_a_leader() {
        // Parallel lines either side of the route: rather than cover one,
        // the label sits just clear of them, joined to its line by a leader.
        let path = [(0.0, 0.0), (0.0, 100.0)];
        let lines: Vec<Segment> = [-20.0, -8.0, 8.0, 20.0]
            .iter()
            .map(|&x| ((x, -500.0), (x, 500.0)))
            .collect();
        let spot = place_label(&path, (40.0, 14.0), &index(&[], &lines), &|_| true);
        let r = rect_at(spot.centre, (40.0, 14.0));
        assert!(
            lines
                .iter()
                .all(|l| segment_rect_overlap(l.0, l.1, r) == 0.0),
            "{spot:?}"
        );
        let (_, on_line) = spot.leader.expect("a leader");
        assert_eq!(on_line.0, 0.0);
        // Too far to step aside: it stays on its line, covering others.
        let walls = [
            (-300.0, -500.0, 270.0, 1100.0),
            (30.0, -500.0, 270.0, 1100.0),
        ];
        let spot = place_label(&path, (40.0, 14.0), &index(&walls, &lines), &|_| true);
        assert_eq!(spot.leader, None);
        assert!(spot.centre.0.abs() < 8.0);
    }

    #[test]
    fn ignores_lines_it_is_told_are_its_own() {
        let path = [(0.0, 0.0), (100.0, 0.0)];
        let mut o = Obstacles::default();
        o.add_segment(((50.0, -50.0), (50.0, 50.0)), Some(3));
        let p = place_label(&path, SIZE, &o, &|owner| owner != Some(3));
        assert_eq!(p.centre, (50.0, 0.0));
        assert_eq!(p.leader, None);
    }
}
