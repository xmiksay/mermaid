//! Where a detour may go into an edge's control points: between the end
//! runs that keep heads square-on and straight, with free ends aimed again.

use super::super::edges::{clip_end, EndClip};
use super::super::round::is_round;
use super::super::route::Pt;
use super::body::{dist, dot, unit};
use super::DetourEdge;

/// The run kept straight off an unmarked end.
const BARE_RUN: f64 = 8.0;
/// The closest a slid tip comes to a rect's corner.
const CORNER: f64 = 8.0;
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
    /// Marked ends whose run had to go: their lead-ins are laid again
    /// toward the detour.
    cut: [bool; 2],
}

impl Room {
    /// A detoured `c` built on `pts`: a lengthened run whose detour does not
    /// start right after it goes back to what it was (the extra points
    /// would bend the curve there for nothing), and a free end is aimed
    /// again from its new neighbour.
    pub(super) fn finish(
        &self,
        mut c: Vec<Pt>,
        edge: &DetourEdge,
        runs: bool,
        free_ok: &dyn Fn(usize, Pt, Pt) -> bool,
    ) -> Vec<Vec<Pt>> {
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
        let mut out = vec![c];
        for (which, (node, lead)) in edge.clips.iter().zip(edge.leads).enumerate() {
            let lead = if runs { lead } else { 0.0 };
            if self.cut[which] {
                out.iter_mut()
                    .for_each(|c| reaim(c, which, None, lead, false));
            } else if !self.free[which] {
            } else if node.shape.is_some_and(is_round) {
                // Aimed again, a round end must not land by another edge's
                // head; there it keeps its tip and only turns its run.
                for c in &mut out {
                    let mut aimed = c.clone();
                    reaim(&mut aimed, which, Some(node), lead, false);
                    let tip = if which == 0 {
                        aimed.first()
                    } else {
                        aimed.last()
                    };
                    if tip.is_some_and(|&t| free_ok(which, t, t)) {
                        *c = aimed;
                    } else {
                        reaim(c, which, None, lead, false);
                    }
                }
            } else {
                // A bare rect end slides along its face toward the detour, or
                // turns onto the face it now heads for — each tried where it
                // leaves clear of the nodes around and of the heads on its
                // own node — else stays.
                out = out
                    .into_iter()
                    .flat_map(|c| {
                        let mut ways: Vec<Vec<Pt>> = [false, true]
                            .into_iter()
                            .map(|turn| {
                                let mut aimed = c.clone();
                                reaim(&mut aimed, which, Some(node), lead, turn);
                                aimed
                            })
                            .filter(|aimed| {
                                let n = aimed.len();
                                let out = if which == 0 {
                                    (aimed.first(), aimed.get(1))
                                } else {
                                    (aimed.last(), n.checked_sub(2).and_then(|i| aimed.get(i)))
                                };
                                matches!(out, (Some(&a), Some(&b)) if free_ok(which, a, b))
                            })
                            .collect();
                        ways.dedup();
                        if ways.is_empty() {
                            ways.push(c);
                        }
                        ways
                    })
                    .collect();
            }
        }
        out
    }
}

/// The `Room` for `ctrl` (with `runs`; else only the tips are kept). A
/// free end — round, or unmarked: nothing to keep square or straight —
/// drops its run down to its tip, and `reaim` rebuilds it toward the
/// detour, so the curve leaves it heading where it goes.
pub(super) fn room(
    ctrl: &[Pt],
    edge: &DetourEdge,
    runs: bool,
    clear: &dyn Fn(Pt, Pt) -> bool,
) -> Option<Room> {
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
            cut: [false; 2],
        });
    }
    let rev: Vec<Pt> = ctrl.iter().rev().copied().collect();
    let (ks, xs, rs) = end_run(ctrl, leads[0], clear)?;
    let (kt, xt, rt) = end_run(&rev, leads[1], clear)?;
    // A marked run that had to stop short would bend under the marker:
    // drop it and lay it again, straight, toward the detour.
    let cut = [(rs, 0), (rt, 1)].map(|(r, w)| !free[w] && r < 2.0 * leads[w] - 1.0);
    let drop = [free[0] || cut[0], free[1] || cut[1]];
    if ks + kt + 2 > n || (n == 2 && rs + rt + 2.0 >= dist(ctrl[0], ctrl[1])) {
        return None;
    }
    let mut extra = [(0, 0); 2];
    let mut out: Vec<Pt> = ctrl[..1].to_vec();
    if !drop[0] {
        out.extend(&ctrl[1..=ks]);
        extra[0] = (out.len(), xs.len());
        out.extend(&xs);
    }
    let lo = out.len() - 1;
    out.extend(&ctrl[ks + 1..n - 1 - kt]);
    if !drop[1] {
        extra[1] = (kt + 1, xt.len());
        out.extend(xt.iter().rev());
        out.extend(&ctrl[n - 1 - kt..n - 1]);
    }
    out.extend(&ctrl[n - 1..]);
    let tail = if drop[1] { 0 } else { kt + xt.len() };
    let hi = (out.len() - tail).checked_sub(2)?;
    (lo <= hi).then_some(Room {
        pts: out,
        lo,
        hi,
        free,
        extra,
        cut,
    })
}

/// Clip end `which` again on the ray from its new neighbour toward the
/// centre of `node` (`None`: keep the tip where it is), as `route` does,
/// with its straight run (one and two lead-ins on a marked end) on that
/// ray; a bare rect end slides along its face instead, or with `turn` is
/// clipped as a round one and kept off the corners.
fn reaim(ctrl: &mut Vec<Pt>, which: usize, node: Option<&EndClip>, lead: f64, turn: bool) {
    if which == 1 {
        ctrl.reverse();
    }
    if let (Some(&next), Some(&old)) = (ctrl.get(1), ctrl.first()) {
        let tip = match node {
            None => old,
            Some(n) if n.shape.is_some_and(is_round) => clip_end(next, n),
            // A bare rect end slides along the face it left from toward its
            // new neighbour, or (`turn`) leaves the face it heads for.
            Some(n) if turn => {
                let t = clip_end(next, n);
                slide(t, t, n)
            }
            Some(n) => slide(old, next, n),
        };
        // A marked end's run must reach its full length to keep the line
        // straight under the marker: waypoints it would pass give way (the
        // far end never does).
        if lead > 0.0 {
            while ctrl.len() > 3
                && ctrl
                    .get(1)
                    .is_some_and(|&p| dist(tip, p) < 2.0 * lead + MIN_SPACING)
            {
                ctrl.remove(1);
            }
        }
        let run: Vec<Pt> = match unit(tip, next) {
            Some(u) if lead > 0.0 => [lead, 2.0 * lead]
                .map(|t| (tip.0 + u.0 * t, tip.1 + u.1 * t))
                .to_vec(),
            _ => Vec::new(),
        };
        ctrl[0] = tip;
        ctrl.splice(1..1, run);
    }
    if which == 1 {
        ctrl.reverse();
    }
}

/// `tip` moved along the rect face it lies on toward `toward`, kept
/// `CORNER` clear of the face's corners so the edge does not leave right at
/// one.
fn slide(tip: Pt, toward: Pt, node: &EndClip) -> Pt {
    let (hw, hh) = (node.size.0 / 2.0, node.size.1 / 2.0);
    let (dx, dy) = (tip.0 - node.center.0, tip.1 - node.center.1);
    let clamp = |v: f64, h: f64| {
        let m = (h - CORNER).max(0.0);
        v.max(-m).min(m)
    };
    // The face the tip lies on: the one it is relatively furthest out on.
    if dx.abs() * hh >= dy.abs() * hw {
        (tip.0, node.center.1 + clamp(toward.1 - node.center.1, hh))
    } else {
        (node.center.0 + clamp(toward.0 - node.center.0, hw), tip.1)
    }
}

/// The straight run off the end at `pts[0]` that a detour leaves alone: the
/// collinear points already there (a lead-in and the point beyond it), then
/// new points on the same ray out to one and two lead-ins on a marked end —
/// enough to keep the curve straight under the marker — or `BARE_RUN` on an
/// unmarked one. The run stops short of any stretch that is not `clear` (a
/// thick stroke's long lead-ins could reach into a neighbouring node, where
/// no detour could then help). Returns how many existing points it keeps,
/// the new points and how far it reaches.
fn end_run(pts: &[Pt], lead: f64, clear: &dyn Fn(Pt, Pt) -> bool) -> Option<(usize, Vec<Pt>, f64)> {
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
        if off(p) >= 0.5 || a <= reach || a > want + 1.0 || !clear(tip, p) {
            break;
        }
        kept += 1;
        reach = a;
    }
    let at = |t: f64| (tip.0 + u.0 * t, tip.1 + u.1 * t);
    let extra: Vec<Pt> = targets
        .into_iter()
        .filter(|&t| t > reach + 1.0)
        .take_while(|&t| clear(tip, at(t)))
        .map(at)
        .collect();
    let reach = extra.last().map_or(reach, |&p| along(p));
    Some((kept, extra, reach))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::NodeShape;

    #[test]
    fn a_run_stops_short_of_what_it_would_run_into() {
        // Tip at x 0 heading right; nothing is clear past x 30.
        let pts = [(0.0, 0.0), (100.0, 0.0), (300.0, 0.0)];
        let clear = |_: Pt, b: Pt| b.0 <= 30.0;
        let (kept, extra, reach) = end_run(&pts, 22.0, &clear).expect("run");
        assert_eq!((kept, extra.len()), (0, 1), "one lead-in, not two");
        assert!((reach - 22.0).abs() < 1e-9);
        let all = |_: Pt, _: Pt| true;
        let (_, extra, reach) = end_run(&pts, 22.0, &all).expect("run");
        assert_eq!(extra.len(), 2);
        assert!((reach - 44.0).abs() < 1e-9);
    }

    #[test]
    fn a_bare_tip_slides_along_its_face_and_off_the_corners() {
        let node = EndClip {
            center: (0.0, 0.0),
            size: (60.0, 40.0),
            shape: Some(NodeShape::Rect),
        };
        // On the right face, toward a point up and to the right: slides up,
        // but stops 8px short of the corner.
        assert_eq!(slide((30.0, 0.0), (100.0, -100.0), &node), (30.0, -12.0));
        // On the top face, toward a point left: slides left.
        assert_eq!(slide((0.0, -20.0), (-10.0, -100.0), &node), (-10.0, -20.0));
    }
}
