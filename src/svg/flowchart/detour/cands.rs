//! Detoured control points to try around one node an edge runs through.

use crate::parse::EdgeCurve;

use super::super::route::Pt;
use super::body::{dist, dot, unit, Body};
use super::room::{room, Room, MIN_SPACING};
use super::scene::Hit;
use super::DetourEdge;

/// How far beyond the node a detour runs, in node gaps: mid-gap first.
const OFFSETS: [f64; 3] = [0.5, 1.0, 2.0];

/// One way round, not built yet: built only when its `shift` (how far it
/// moves the curve aside) can still beat the best one found.
pub(super) enum Way {
    /// One waypoint (`single`) or a pair beside the crossing: through
    /// `side`, ±`w` along the crossing direction `d`.
    Beside {
        side: Pt,
        d: Pt,
        w: f64,
        single: bool,
    },
    /// Waypoints out from the node's corners.
    Corners(Vec<Pt>),
    /// A step curve's segment `i` led through `a` and `z`.
    Step { i: usize, a: Pt, z: Pt },
}

impl Way {
    /// The same way with its waypoints moved by `v`.
    pub(super) fn shifted(&self, v: Pt) -> Way {
        let mv = |p: Pt| (p.0 + v.0, p.1 + v.1);
        match self {
            Way::Beside { side, d, w, single } => Way::Beside {
                side: mv(*side),
                d: *d,
                w: *w,
                single: *single,
            },
            Way::Corners(run) => Way::Corners(run.iter().map(|&p| mv(p)).collect()),
            Way::Step { i, a, z } => Way::Step {
                i: *i,
                a: mv(*a),
                z: mv(*z),
            },
        }
    }

    /// Where its waypoints sit, on average.
    pub(super) fn centre(&self) -> Pt {
        match self {
            Way::Beside { side, .. } => *side,
            Way::Corners(run) => {
                let k = run.len().max(1) as f64;
                let s = run.iter().fold((0.0, 0.0), |m, p| (m.0 + p.0, m.1 + p.1));
                (s.0 / k, s.1 / k)
            }
            Way::Step { a, z, .. } => ((a.0 + z.0) / 2.0, (a.1 + z.1) / 2.0),
        }
    }
}

/// The ways round one node, least sideways movement first: the close ones,
/// then — tried only when none of those works — the wide ones.
pub(super) struct Plan<'c> {
    pub(super) ways: [Vec<(f64, Way)>; 2],
    room: Option<Room>,
    runs: bool,
    /// Whether a re-aimed bare end `(which, tip, next)` may leave there.
    free_ok: &'c dyn Fn(usize, Pt, Pt) -> bool,
}

impl Plan<'_> {
    /// The control points of `way` built on `ctrl`.
    pub(super) fn build(&self, ctrl: &[Pt], way: &Way, edge: &DetourEdge) -> Option<Vec<Pt>> {
        match way {
            Way::Step { i, a, z } => {
                let mut c = ctrl.to_vec();
                let at = (i + 1).min(c.len());
                c.splice(at..at, [*a, *z]);
                Some(c)
            }
            Way::Beside { side, d, w, single } => {
                let room = self.room.as_ref()?;
                let c = insert(&room.pts, (room.lo, room.hi), *side, *d, *w, *single)?;
                Some(room.finish(c, edge, self.runs, self.free_ok))
            }
            Way::Corners(run) => {
                let room = self.room.as_ref()?;
                let c = insert_run(&room.pts, (room.lo, room.hi), run)?;
                Some(room.finish(c, edge, self.runs, self.free_ok))
            }
        }
    }
}

/// What a plan asks of the scene: whether a straight stretch keeps clear of
/// every foreign node, and whether a re-aimed bare end `(which, tip, next)`
/// may leave there.
pub(super) struct Checks<'c> {
    pub(super) clear: &'c dyn Fn(Pt, Pt) -> bool,
    pub(super) free_ok: &'c dyn Fn(usize, Pt, Pt) -> bool,
}

/// Every way round the node `body` the curve runs through at `h`, least
/// sideways movement first.
pub(super) fn candidates<'c>(
    ctrl: &[Pt],
    samples: &[Pt],
    h: &Hit,
    body: &Body,
    edge: &DetourEdge,
    (curve, gap): (EdgeCurve, f64),
    checks: Checks<'c>,
) -> Plan<'c> {
    let Checks { clear, free_ok } = checks;
    let runs = curve != EdgeCurve::Linear;
    let mut plan = Plan {
        ways: [Vec::new(), Vec::new()],
        room: None,
        runs,
        free_ok,
    };
    let (Some(&entry), Some(&exit)) = (
        samples.get(h.first),
        samples.get(h.last + 1).or(samples.last()),
    ) else {
        return plan;
    };
    let (close, wide) = OFFSETS.split_at(2);
    if curve == EdgeCurve::Step {
        plan.ways = [close, wide].map(|o| steps(ctrl, (entry, exit), h, body, gap, o));
    } else if let Some(d) = unit(entry, exit) {
        plan.room = room(ctrl, edge, runs, clear);
        plan.ways = [close, wide].map(|o| beside(ctrl, (entry, exit), d, body, gap, o));
        plan.ways[1].extend(corners(ctrl, (entry, exit), d, body, edge, gap));
    }
    for ways in &mut plan.ways {
        ways.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    plan
}

/// Basis and linear curves: one waypoint, or a pair, beside the stretch
/// the curve crosses the node on, perpendicular to that crossing. The
/// shift is measured from the crossing on the node's actual outline near
/// it, so clipping a wide node's corner moves the curve by the corner, not
/// by half the node.
fn beside(
    ctrl: &[Pt],
    (entry, exit): (Pt, Pt),
    d: Pt,
    body: &Body,
    gap: f64,
    offsets: &[f64],
) -> Vec<(f64, Way)> {
    let span = dist(
        ctrl.first().copied().unwrap_or(entry),
        ctrl.last().copied().unwrap_or(exit),
    );
    let n = (-d.1, d.0);
    let mid = ((entry.0 + exit.0) / 2.0, (entry.1 + exit.1) / 2.0);
    let half = dist(entry, exit) / 2.0;
    let mut out = Vec::new();
    for &k in offsets {
        let clear = k * gap;
        let w = half + clear / 2.0;
        for s in [1.0, -1.0] {
            let nn = (n.0 * s, n.1 * s);
            let Some(req) = body.reach(mid, d, nn, w).map(|r| r.max(0.0) + clear) else {
                continue;
            };
            if req > span.max(gap) {
                continue;
            }
            let side = (mid.0 + nn.0 * req, mid.1 + nn.1 * req);
            for single in [true, false] {
                out.push((req, Way::Beside { side, d, w, single }));
            }
        }
    }
    out
}

/// Basis and linear curves round a corner of the node, or along one side
/// past two: needed where the far end sits behind a wide node, so the curve
/// has to go round the node's end and come back under it. Waypoints sit a
/// clearance out from each corner, in flow order.
fn corners(
    ctrl: &[Pt],
    (entry, exit): (Pt, Pt),
    d: Pt,
    body: &Body,
    edge: &DetourEdge,
    gap: f64,
) -> Vec<(f64, Way)> {
    let corners = body.corners();
    let mid = ((entry.0 + exit.0) / 2.0, (entry.1 + exit.1) / 2.0);
    let n = (-d.1, d.0);
    let flow = |p: Pt| if edge.flow == 0 { p.0 } else { p.1 };
    let forward = match (ctrl.first(), ctrl.last()) {
        (Some(&a), Some(&b)) if flow(b) < flow(a) => -1.0,
        _ => 1.0,
    };
    let mut out = Vec::new();
    for k in OFFSETS {
        let c = k * gap;
        let pts: Vec<Pt> = corners
            .iter()
            .map(|&(v, o)| (v.0 + o.0 * c, v.1 + o.1 * c))
            .collect();
        let next = pts.iter().cycle().skip(1);
        let runs = pts
            .iter()
            .map(|&p| vec![p])
            .chain(pts.iter().zip(next).map(|(&p, &q)| vec![p, q]));
        for mut run in runs {
            run.sort_by(|a, b| (forward * flow(*a)).total_cmp(&(forward * flow(*b))));
            let shift = run
                .iter()
                .map(|p| dot((p.0 - mid.0, p.1 - mid.1), n).abs())
                .fold(0.0, f64::max);
            out.push((shift, Way::Corners(run)));
        }
    }
    out
}

/// `ctrl` with `run` put between two points `i < j` in `lo..=hi + 1`, in
/// place of the (at most one) point between them: the one adding the least
/// length, keeping `MIN_SPACING` from the points either side.
fn insert_run(ctrl: &[Pt], (lo, hi): (usize, usize), run: &[Pt]) -> Option<Vec<Pt>> {
    let (&first, &last) = (run.first()?, run.last()?);
    let inner: f64 = run.windows(2).map(|w| dist(w[0], w[1])).sum();
    let mut best: Option<(f64, usize, usize)> = None;
    for i in lo..=hi {
        for j in i + 1..=(i + 2).min(hi + 1) {
            let (Some(&p), Some(&q), Some(old)) = (ctrl.get(i), ctrl.get(j), ctrl.get(i..=j))
            else {
                continue;
            };
            if dist(p, first) < MIN_SPACING || dist(q, last) < MIN_SPACING {
                continue;
            }
            let was: f64 = old.windows(2).map(|w| dist(w[0], w[1])).sum();
            let cost = dist(p, first) + inner + dist(last, q) - was;
            if cost.is_finite() && best.is_none_or(|b| cost < b.0) {
                best = Some((cost, i, j));
            }
        }
    }
    let (_, i, j) = best?;
    let mut out = ctrl.get(..=i)?.to_vec();
    out.extend(run);
    out.extend(ctrl.get(j..)?);
    Some(out)
}

/// Step curves draw each control segment as horizontal, vertical at its
/// mid-x, horizontal: a pair of waypoints on one vertical line `X` makes
/// that line the vertical leg, a pair on one horizontal line `Y` (below
/// and above the segment's ends) the horizontal leg. Both are put a
/// clearance clear of the node's box.
fn steps(
    ctrl: &[Pt],
    (entry, exit): (Pt, Pt),
    h: &Hit,
    body: &Body,
    gap: f64,
    offsets: &[f64],
) -> Vec<(f64, Way)> {
    let m = ((entry.0 + exit.0) / 2.0, (entry.1 + exit.1) / 2.0);
    let b = body.bbox;
    let last = ctrl.len().saturating_sub(2);
    let mut out = Vec::new();
    // `drawn` gives three points per control segment after the first.
    for i in (h.first / 3).min(last)..=(h.last / 3).min(last) {
        let (Some(&p), Some(&q)) = (ctrl.get(i), ctrl.get(i + 1)) else {
            continue;
        };
        for &k in offsets {
            let c = k * gap;
            for x in [b[0] - c, b[2] + c] {
                out.push((
                    (x - m.0).abs(),
                    Way::Step {
                        i,
                        a: (x, p.1),
                        z: (x, q.1),
                    },
                ));
            }
            for y in [b[1] - c, b[3] + c] {
                out.push((
                    (y - m.1).abs(),
                    Way::Step {
                        i,
                        a: (p.0, y),
                        z: (q.0, y),
                    },
                ));
            }
        }
    }
    out
}

/// `ctrl` with the detour run beside the node — through `side`, ±`along`
/// on the crossing direction `d` — put between two points `i < j` in
/// `lo..=hi + 1` that head along `d` past the node, in place of the (up to
/// `MAX_DROP`) points between them, clamped to their stretch along `d` so
/// the curve never runs backwards: the shortest result, then the one
/// dropping fewest points.
fn insert(
    ctrl: &[Pt],
    (lo, hi): (usize, usize),
    side: Pt,
    d: Pt,
    along: f64,
    single: bool,
) -> Option<Vec<Pt>> {
    const MAX_DROP: usize = 1;
    let at = |t: f64| (side.0 + d.0 * t, side.1 + d.1 * t);
    let proj = |p: Pt| dot((p.0 - side.0, p.1 - side.1), d);
    let length = |pts: &[Pt]| pts.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>();
    let mut found: Vec<(f64, usize, usize, Vec<Pt>)> = Vec::new();
    for i in lo..=hi {
        for j in i + 1..=(i + 1 + MAX_DROP).min(hi + 1) {
            let (Some(&p), Some(&q), Some(old)) = (ctrl.get(i), ctrl.get(j), ctrl.get(i..=j))
            else {
                continue;
            };
            let (ti, tj) = (proj(p), proj(q));
            if !(ti < along && tj > -along && ti < tj) {
                continue;
            }
            let (t1, t2) = ((-along).max(ti), along.min(tj));
            let run = if single || t2 - t1 < MIN_SPACING {
                vec![at(0.0_f64.max(t1).min(t2))]
            } else {
                vec![at(t1), at(t2)]
            };
            // A waypoint right by a kept one bends the curve to a corner.
            let run: Vec<Pt> = run
                .into_iter()
                .filter(|&r| dist(r, p) >= MIN_SPACING && dist(r, q) >= MIN_SPACING)
                .collect();
            if run.is_empty() {
                continue;
            }
            let mut new = vec![p];
            new.extend(&run);
            new.push(q);
            let cost = length(&new) - length(old);
            if cost.is_finite() {
                let mut out = ctrl[..=i].to_vec();
                out.extend(run);
                out.extend(&ctrl[j..]);
                found.push((cost, j - i, i, out));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    found.into_iter().next().map(|f| f.3)
}
