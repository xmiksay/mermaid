//! Where a detour may go into an edge's control points: between the end
//! runs that keep heads square-on and straight, with free ends aimed again.

use super::super::edges::{clip_end, EndClip};
use super::super::round::is_round;
use super::super::route::Pt;
use super::body::{dist, dot, unit};
use super::DetourEdge;

/// The run kept straight off an unmarked end.
const BARE_RUN: f64 = 8.0;
/// The closest a detour waypoint comes to a neighbouring control point.
pub(super) const MIN_SPACING: f64 = 10.0;

/// `ctrl` with each end's straight run marked off, and the range of
/// segments detour waypoints may go into: between the two runs.
pub(super) struct Room {
    pub(super) pts: Vec<Pt>,
    pub(super) lo: usize,
    pub(super) hi: usize,
    /// Ends aimed again from their new neighbour.
    free: [bool; 2],
    /// The points added to lengthen each run: `(index, count)` at the
    /// start, `(points after them, count)` at the end.
    extra: [(usize, usize); 2],
}

impl Room {
    /// A detoured `c` built on `pts`: a lengthened run whose detour does not
    /// start right after it goes back to what it was (the extra points
    /// would bend the curve there for nothing), and a free end is aimed
    /// again from its new neighbour.
    pub(super) fn finish(&self, mut c: Vec<Pt>, edge: &DetourEdge, runs: bool) -> Vec<Pt> {
        let new = |p: Option<&Pt>| p.is_some_and(|p| !self.pts.contains(p));
        let [(b, k), (a, m)] = self.extra;
        if m > 0 {
            if let Some(start) = c.len().checked_sub(a + m) {
                if !new(start.checked_sub(1).and_then(|i| c.get(i))) {
                    c.drain(start..start + m);
                }
            }
        }
        if k > 0 && !new(c.get(b + k)) {
            c.drain(b..b + k);
        }
        for (which, (node, lead)) in edge.clips.iter().zip(edge.leads).enumerate() {
            if self.free[which] {
                reaim(&mut c, which, node, if runs { lead } else { 0.0 });
            }
        }
        c
    }
}

/// The `Room` for `ctrl` (with `runs`; else only the tips are kept). A
/// free end — round, or unmarked: nothing to keep square or straight —
/// drops its run down to its tip, and `reaim` rebuilds it toward the
/// detour, so the curve leaves it heading where it goes.
pub(super) fn room(ctrl: &[Pt], edge: &DetourEdge, runs: bool) -> Option<Room> {
    let leads = edge.leads;
    let free = [0, 1].map(|w| leads[w] <= 0.0 || edge.clips[w].shape.is_some_and(is_round));
    let n = ctrl.len();
    if !runs {
        return (n >= 2).then(|| Room {
            pts: ctrl.to_vec(),
            lo: 0,
            hi: n - 2,
            free,
            extra: [(0, 0); 2],
        });
    }
    let rev: Vec<Pt> = ctrl.iter().rev().copied().collect();
    let (ks, xs, rs) = end_run(ctrl, leads[0])?;
    let (kt, xt, rt) = end_run(&rev, leads[1])?;
    if ks + kt + 2 > n || (n == 2 && rs + rt + 2.0 >= dist(ctrl[0], ctrl[1])) {
        return None;
    }
    let mut extra = [(0, 0); 2];
    let mut out: Vec<Pt> = ctrl[..1].to_vec();
    if !free[0] {
        out.extend(&ctrl[1..=ks]);
        extra[0] = (out.len(), xs.len());
        out.extend(&xs);
    }
    let lo = out.len() - 1;
    out.extend(&ctrl[ks + 1..n - 1 - kt]);
    if !free[1] {
        extra[1] = (kt + 1, xt.len());
        out.extend(xt.iter().rev());
        out.extend(&ctrl[n - 1 - kt..n - 1]);
    }
    out.extend(&ctrl[n - 1..]);
    let tail = if free[1] { 0 } else { kt + xt.len() };
    let hi = (out.len() - tail).checked_sub(2)?;
    (lo <= hi).then_some(Room {
        pts: out,
        lo,
        hi,
        free,
        extra,
    })
}

/// Clip round end `which` again on the ray from its new neighbour toward
/// the centre, as `route` does, with its straight run (one and two lead-ins
/// on a marked end) on that ray, each kept `MIN_SPACING` short of the
/// neighbour so the curve does not corner there.
fn reaim(ctrl: &mut Vec<Pt>, which: usize, node: &EndClip, lead: f64) {
    if which == 1 {
        ctrl.reverse();
    }
    if let Some(&next) = ctrl.get(1) {
        let tip = clip_end(next, node);
        let run: Vec<Pt> = match unit(tip, next) {
            Some(u) if lead > 0.0 => [lead, 2.0 * lead]
                .into_iter()
                .filter(|&t| t + MIN_SPACING <= dist(tip, next))
                .map(|t| (tip.0 + u.0 * t, tip.1 + u.1 * t))
                .collect(),
            _ => Vec::new(),
        };
        ctrl[0] = tip;
        ctrl.splice(1..1, run);
    }
    if which == 1 {
        ctrl.reverse();
    }
}

/// The straight run off the end at `pts[0]` that a detour leaves alone: the
/// collinear points already there (a lead-in and the point beyond it), then
/// new points on the same ray out to one and two lead-ins on a marked end —
/// enough to keep the curve straight under the marker — or `BARE_RUN` on an
/// unmarked one. Returns how many existing points it keeps, the new points
/// and how far it reaches.
fn end_run(pts: &[Pt], lead: f64) -> Option<(usize, Vec<Pt>, f64)> {
    let (&tip, &next) = (pts.first()?, pts.get(1)?);
    let u = unit(tip, next)?;
    let along = |p: Pt| dot((p.0 - tip.0, p.1 - tip.1), u);
    let off = |p: Pt| ((p.0 - tip.0) * u.1 - (p.1 - tip.1) * u.0).abs();
    let targets = if lead > 0.0 {
        vec![lead, 2.0 * lead]
    } else {
        vec![BARE_RUN]
    };
    let want = targets.last().copied().unwrap_or(0.0);
    let (mut kept, mut reach) = (0, 0.0);
    // The far end is never part of this end's run.
    for &p in pts.iter().take(pts.len() - 1).skip(1).take(2) {
        let a = along(p);
        if off(p) >= 0.5 || a <= reach || a > want + 1.0 {
            break;
        }
        kept += 1;
        reach = a;
    }
    let extra: Vec<Pt> = targets
        .into_iter()
        .filter(|&t| t > reach + 1.0)
        .map(|t| (tip.0 + u.0 * t, tip.1 + u.1 * t))
        .collect();
    Some((kept, extra, reach.max(want)))
}
