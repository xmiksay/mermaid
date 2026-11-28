//! Judging one candidate way round: what it newly runs through or comes
//! close to, how far it moves the curve, and its score.

use std::ops::Range;

use crate::parse::EdgeCurve;

use super::super::route::Pt;
use super::body::{dist, drawn, unit, Body, SAMPLES};
use super::scene::{Hit, Ob, Scene, CLOSE};
use super::DetourEdge;

/// A curve this close to a node grazes it.
pub(super) const GRAZE: f64 = 6.0;
/// How near a bezier must come to a node or marker to be sampled finely:
/// the widest test made of a curve (`GRAZE` round a body grown by `CLOSE`).
pub(super) const NEAR_ANY: f64 = GRAZE + CLOSE + 1.0;
/// Score per node newly come `CLOSE` to: more than any clean way round
/// costs, so that is taken only when nothing else clears the node.
pub(super) const GRAZE_COST: f64 = 1000.0;
/// Score per node newly grazed, but not that close.
const SOFT_COST: f64 = 40.0;
/// Score per other edge's marker newly run over.
const MARK_COST: f64 = 150.0;
/// Score per px of running along another line (for stretches past `RUN`).
const ALONG_COST: f64 = 4.0;
/// Slack on how far a detour may run back against the edge's direction.
pub(super) const BACKTRACK: f64 = 10.0;
/// Whether a polyline doubles back on itself: a leg followed by one
/// running the opposite way along the same line.
fn retraces(pts: &[Pt]) -> bool {
    let legs: Vec<Pt> = pts
        .windows(2)
        .map(|w| (w[1].0 - w[0].0, w[1].1 - w[0].1))
        .filter(|d| d.0.abs() + d.1.abs() > 1e-9)
        .collect();
    legs.windows(2).any(|l| {
        let (a, b) = (l[0], l[1]);
        (a.0 * b.1 - a.1 * b.0).abs() < 1e-9 && a.0 * b.0 + a.1 * b.1 < 0.0
    })
}

/// How many leading and trailing samples `a` and `b` share.
fn shared(a: &[Pt], b: &[Pt]) -> (usize, usize) {
    let pre = a.iter().zip(b).take_while(|(p, q)| p == q).count();
    let room = a.len().min(b.len()) - pre;
    let post = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(room)
        .take_while(|(p, q)| p == q)
        .count();
    (pre, post)
}

/// Length and backtrack of a sampled curve: travel against the direction
/// the edge heads along the flow axis `flow`, from tip to tip (`tips`; along
/// their line when both sit in one rank). Going round the end of a wide
/// node may run back across the flow, never against it.
pub(super) fn measure(samples: &[Pt], flow: usize, (a, b): (Pt, Pt)) -> (f64, f64) {
    let along = if flow == 0 { b.0 - a.0 } else { b.1 - a.1 };
    let u = if along.abs() >= 1.0 {
        let s = along.signum();
        Some(if flow == 0 { (s, 0.0) } else { (0.0, s) })
    } else {
        unit(a, b)
    };
    samples.windows(2).fold((0.0, 0.0), |(len, back), w| {
        let step = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        let along = u.map_or(0.0, |u| step.0 * u.0 + step.1 * u.1);
        (len + dist(w[0], w[1]), back + (-along).max(0.0))
    })
}

/// A way round worth keeping: its score, control points, samples, length
/// and backtrack, and the nodes it newly grazes. A clean one is first
/// scored without running along other lines, the costliest part: `along`
/// is the stretch of samples that still has to be scored for it, if any.
pub(super) struct Pick {
    pub(super) score: f64,
    pub(super) ctrl: Vec<Pt>,
    pub(super) samples: Vec<Pt>,
    pub(super) moved: (f64, f64),
    pub(super) grazes: Vec<Ob>,
    along: Option<Range<usize>>,
}

impl Pick {
    /// The score in full: `along` the stretch it still has to be scored on.
    fn settle(&mut self, along: &dyn Fn(&[Pt]) -> f64) {
        if let Some(r) = self.along.take() {
            self.score += ALONG_COST * along(self.samples.get(r).unwrap_or(&self.samples));
        }
    }
}

/// The best clean way so far, and the grazing ones among those settled.
pub(super) type Best<'w, W> = (Option<(Pick, &'w W)>, Vec<(f64, Vec<Ob>, &'w W)>);

/// Settle the clean ways in `pending` whose partial score is below `upto`,
/// least first, while one may still beat the best: the rest cannot.
pub(super) fn settle<'w, W>(
    pending: &mut Vec<(Pick, &'w W)>,
    (best, grazing): &mut Best<'w, W>,
    upto: f64,
    along: &dyn Fn(&[Pt]) -> f64,
) {
    pending.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
    loop {
        let bound = best.as_ref().map_or(f64::INFINITY, |b| b.0.score);
        match pending.last() {
            Some(p) if p.0.score >= bound => pending.clear(),
            Some(p) if p.0.score < upto => {}
            _ => break,
        }
        let Some((mut p, way)) = pending.pop() else {
            break;
        };
        p.settle(along);
        if p.score < bound {
            if !p.grazes.is_empty() {
                grazing.push((p.score, p.grazes.clone(), way));
            }
            *best = Some((p, way));
        }
    }
}

pub(super) enum Verdict {
    No,
    /// Clears the node but runs through one other, never passed before.
    Trade(Pick),
    Clean(Pick),
}

/// One edge's search.
pub(super) struct Search<'s, 'a, 'e> {
    pub(super) scene: &'s Scene<'a>,
    pub(super) edge: &'s DetourEdge<'e>,
    pub(super) curve: EdgeCurve,
    pub(super) gap: f64,
    pub(super) tips: (Pt, Pt),
    /// Nodes stacked on an end, which the edge may run through.
    pub(super) stacked: &'s [&'s str],
    pub(super) len0: f64,
    pub(super) back0: f64,
}

/// One round: the curve so far and the node it is being taken round.
pub(super) struct Round<'r> {
    pub(super) samples: &'r [Pt],
    pub(super) h: &'r Hit,
    /// What else the curve runs through now (nodes and markers).
    pub(super) rest: Vec<Ob>,
    /// The nodes it grazes now.
    pub(super) grazed: Vec<Ob>,
    pub(super) passed: &'r [Ob],
    pub(super) moved: (f64, f64),
    /// The round takes the curve off a node it comes close to, not out of
    /// one it runs through.
    pub(super) off: bool,
}

impl Search<'_, '_, '_> {
    pub(super) fn owned(&self) -> impl Fn(&str, Pt) -> bool + '_ {
        |id, c| self.edge.owns(id, c) || self.stacked.contains(&id)
    }

    /// Judge the candidate `ctrl`, which moves the curve `shift` aside.
    pub(super) fn assess(&self, r: &Round, ctrl: Vec<Pt>, shift: f64, bound: f64) -> Verdict {
        let s = drawn(&ctrl, self.curve, &|b| self.scene.near_any(b, NEAR_ANY));
        // Only the stretch that changed can hit anything new; the node
        // being passed must not be hit in what did not change.
        let (pre, post) = shared(r.samples, &s);
        let kept = r.samples.len().saturating_sub(post);
        if r.h.first + 1 < pre || r.h.last >= kept {
            return Verdict::No;
        }
        let span = |v: &[Pt]| pre.saturating_sub(1)..v.len() + 1 - post.max(1);
        let (Some(part), Some(was)) = (s.get(span(&s)), r.samples.get(span(r.samples))) else {
            return Verdict::No;
        };
        if self.curve == EdgeCurve::Step && retraces(part) {
            return Verdict::No;
        }
        let ((nl, nb), (ol, ob)) = (
            measure(part, self.edge.flow, self.tips),
            measure(was, self.edge.flow, self.tips),
        );
        let moved = (r.moved.0 + nl - ol, r.moved.1 + nb - ob);
        if moved.0 > 2.0 * self.len0 + self.gap || moved.1 > self.back0 + BACKTRACK {
            return Verdict::No;
        }
        // Still through the node being passed: told by that node alone,
        // before looking at all the others.
        let through_h = |b: &Body| part.windows(2).any(|w| b.cuts(w[0], w[1]));
        if r.h.ob.0 && !r.off && self.scene.get(r.h.ob).is_some_and(|n| through_h(&n.1)) {
            return Verdict::No;
        }
        let touch = self.scene.touches(part, GRAZE, self.owned());
        let fresh: Vec<&Ob> = touch.cut.iter().filter(|o| !r.rest.contains(o)).collect();
        let new = |v: &[Ob], had: &[Ob]| -> Vec<Ob> {
            v.iter()
                .filter(|o| !had.contains(o) && !fresh.contains(o))
                .copied()
                .collect()
        };
        // Coming close in the stretch that moved counts even next to a node
        // the curve was close to before: it may be closer now.
        let grazes = new(&touch.close, &[]);
        let soft = new(&touch.grazed, &r.grazed)
            .len()
            .saturating_sub(grazes.len());
        // Still through (or close to) the node being passed: no way round.
        if fresh.contains(&&r.h.ob) || (r.off && (!fresh.is_empty() || grazes.contains(&r.h.ob))) {
            return Verdict::No;
        }
        if let [&one] = fresh[..] {
            if r.passed.contains(&one) {
                return Verdict::No;
            }
            return Verdict::Trade(Pick {
                score: shift,
                ctrl,
                samples: s,
                moved,
                grazes,
                along: None,
            });
        }
        // Step and linear curves run straight at what they pass: no grazing.
        if !fresh.is_empty() || (self.curve != EdgeCurve::Basis && grazes.len() + soft > 0) {
            return Verdict::No;
        }
        let new_marks = touch.marks.iter().filter(|o| !r.rest.contains(o)).count();
        let base = shift
            + GRAZE_COST * grazes.len() as f64
            + SOFT_COST * soft as f64
            + MARK_COST * new_marks as f64;
        // Already no better than the best: no need to score running along.
        if base >= bound {
            return Verdict::No;
        }
        // Over the whole stretch that moved, a bezier either side.
        let pad = |k: usize| k.saturating_sub(SAMPLES);
        let along = pad(pre)..s.len() - pad(post);
        Verdict::Clean(Pick {
            score: base,
            ctrl,
            samples: s,
            moved,
            grazes,
            along: Some(along),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(score: f64, along: bool) -> Pick {
        Pick {
            score,
            ctrl: Vec::new(),
            samples: vec![(0.0, 0.0), (1.0, 0.0)],
            moved: (0.0, 0.0),
            grazes: Vec::new(),
            along: along.then_some(0..2),
        }
    }

    #[test]
    fn settling_scores_only_what_may_still_win() {
        // Partial scores 10, 20, 30; running along costs 15 on each.
        let ways = ["a", "b", "c"];
        let mut pending: Vec<(Pick, &&str)> = vec![
            (pick(30.0, true), &ways[2]),
            (pick(10.0, true), &ways[0]),
            (pick(20.0, true), &ways[1]),
        ];
        let mut found: Best<&str> = (None, Vec::new());
        let along = |_: &[Pt]| 15.0 / ALONG_COST;
        // Only those below 15 are settled yet.
        settle(&mut pending, &mut found, 15.0, &along);
        assert_eq!(pending.len(), 2);
        assert_eq!(
            found.0.as_ref().map(|b| (b.0.score, *b.1)),
            Some((25.0, "a"))
        );
        // b (20 + 15) loses to a; c (30) cannot win and is never scored.
        settle(&mut pending, &mut found, f64::INFINITY, &along);
        assert!(pending.is_empty());
        assert_eq!(
            found.0.as_ref().map(|b| (b.0.score, *b.1)),
            Some((25.0, "a"))
        );
    }
}
