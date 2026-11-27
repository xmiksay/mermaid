//! The search for a detour: round by round, the first node the curve still
//! runs through, every way round it scored, the best one kept.

use std::cell::OnceCell;

use crate::parse::EdgeCurve;

use super::super::route::Pt;
use super::body::{dist, drawn, unit, SAMPLES};
use super::cands::{candidates, Checks, Way};
use super::lines::Lines;
use super::scene::{Hit, Ob, Scene};
use super::DetourEdge;

/// Detour rounds per edge, one node each.
const MAX_ROUNDS: usize = 6;
/// A curve this close to a node grazes it.
pub(super) const GRAZE: f64 = 6.0;
/// Score per node newly come `CLOSE` to: more than any clean way round
/// costs, so that is taken only when nothing else clears the node.
const GRAZE_COST: f64 = 1000.0;
/// Score per node newly grazed, but not that close.
const SOFT_COST: f64 = 40.0;
/// Score per other edge's marker newly run over.
const MARK_COST: f64 = 150.0;
/// Score per px of running along another line (for stretches past `RUN`).
const ALONG_COST: f64 = 4.0;
/// Slack on how far a detour may run back against the edge's direction.
pub(super) const BACKTRACK: f64 = 10.0;
/// How far a way that grazes a node is pushed off it, in turn.
const PUSH: [f64; 5] = [3.0, 6.0, 10.0, 15.0, 22.0];
/// How many grazing ways are pushed.
const PUSHED: usize = 3;

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
/// and backtrack, and the nodes it newly grazes.
struct Pick {
    score: f64,
    ctrl: Vec<Pt>,
    samples: Vec<Pt>,
    moved: (f64, f64),
    grazes: Vec<Ob>,
}

enum Verdict {
    No,
    /// Clears the node but runs through one other, never passed before.
    Trade(Pick),
    Clean(Pick),
}

/// One edge's search.
struct Search<'s, 'a, 'e> {
    scene: &'s Scene<'a>,
    edge: &'s DetourEdge<'e>,
    curve: EdgeCurve,
    gap: f64,
    tips: (Pt, Pt),
    len0: f64,
    back0: f64,
}

/// One round: the curve so far and the node it is being taken round.
struct Round<'r> {
    samples: &'r [Pt],
    h: &'r Hit,
    /// What else the curve runs through now (nodes and markers).
    rest: Vec<Ob>,
    /// The nodes it grazes now.
    grazed: Vec<Ob>,
    passed: &'r [Ob],
    moved: (f64, f64),
    /// The lines to score running along, gathered when first needed.
    lines: OnceCell<Lines>,
    region: [f64; 4],
    gather: &'r dyn Fn([f64; 4]) -> Lines,
}

impl Search<'_, '_, '_> {
    fn owned(&self) -> impl Fn(&str, Pt) -> bool + '_ {
        |id, c| self.edge.owns(id, c)
    }

    /// Judge the candidate `ctrl`, which moves the curve `shift` aside.
    fn assess(&self, r: &Round, ctrl: Vec<Pt>, shift: f64, bound: f64) -> Verdict {
        let s = drawn(&ctrl, self.curve);
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
        let ((nl, nb), (ol, ob)) = (
            measure(part, self.edge.flow, self.tips),
            measure(was, self.edge.flow, self.tips),
        );
        let moved = (r.moved.0 + nl - ol, r.moved.1 + nb - ob);
        if moved.0 > 2.0 * self.len0 + self.gap || moved.1 > self.back0 + BACKTRACK {
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
        let along = if base >= bound {
            0.0
        } else {
            // Over the whole stretch that moved, a bezier either side.
            let pad = |k: usize| k.saturating_sub(SAMPLES);
            let lines = r.lines.get_or_init(|| (r.gather)(r.region));
            lines.along(s.get(pad(pre)..s.len() - pad(post)).unwrap_or(&s))
        };
        let score = base + ALONG_COST * along;
        Verdict::Clean(Pick {
            score,
            ctrl,
            samples: s,
            moved,
            grazes,
        })
    }
}

/// How many nodes the sampled curve runs through, and grazes.
fn tally(scene: &Scene, samples: &[Pt], owned: impl Fn(&str, Pt) -> bool) -> (usize, usize) {
    let t = scene.touches(samples, GRAZE, owned);
    (t.cut.len(), t.grazed.len())
}

/// The detoured control points, `None` when the curve hits nothing or no
/// detour leaves it better off. Each round takes the first node still hit
/// (markers never start a round) and keeps the best-scoring way round that
/// clears it without hitting any node the curve did not hit already. The
/// result must run through fewer nodes than the input (or as many and
/// graze fewer).
pub(super) fn detour(
    ctrl: &[Pt],
    curve: EdgeCurve,
    edge: &DetourEdge,
    scene: &Scene,
    gap: f64,
    lines: &dyn Fn([f64; 4]) -> Lines,
) -> Option<Vec<Pt>> {
    let owned = |id: &str, c: Pt| edge.owns(id, c);
    if !scene.may_hit(&super::hull(ctrl), owned) {
        return None;
    }
    let mut samples = drawn(ctrl, curve);
    if !scene.hits_other(&samples, false, 0.0, owned, &[]) {
        return None;
    }
    let tips = (
        ctrl.first().copied().unwrap_or_default(),
        ctrl.last().copied().unwrap_or_default(),
    );
    let (len0, back0) = measure(&samples, edge.flow, tips);
    let search = Search {
        scene,
        edge,
        curve,
        gap,
        tips,
        len0,
        back0,
    };
    let before = tally(scene, &samples, owned);
    let mut moved = (len0, back0);
    let mut hit = scene.hits(&samples, true, 0.0, owned);
    let mut cur = ctrl.to_vec();
    let (mut stuck, mut passed) = (Vec::new(), Vec::new());
    for _ in 0..MAX_ROUNDS {
        let Some(h) = hit.iter().find(|h| h.ob.0 && !stuck.contains(&h.ob)) else {
            break;
        };
        let Some(&(_, body)) = scene.get(h.ob) else {
            break;
        };
        // Grazes the curve already has (nodes it runs through do not count:
        // passing one of those closely later is a new graze).
        let now = scene.touches(&samples, GRAZE, owned);
        let had =
            |v: &[Ob]| -> Vec<Ob> { v.iter().filter(|o| !now.cut.contains(o)).copied().collect() };
        let grazed = had(&now.grazed);
        let round = Round {
            samples: &samples,
            h,
            rest: hit.iter().map(|x| x.ob).filter(|&o| o != h.ob).collect(),
            grazed,
            passed: &passed,
            moved,
            lines: OnceCell::new(),
            // Other edges' lines are looked at near the node: past a few
            // gaps every way round runs alike.
            region: [
                body.bbox[0] - 3.0 * gap,
                body.bbox[1] - 3.0 * gap,
                body.bbox[2] + 3.0 * gap,
                body.bbox[3] + 3.0 * gap,
            ],
            gather: lines,
        };
        let clear = |a: Pt, b: Pt| !scene.hits_other(&[a, b], false, GRAZE, owned, &[]);
        // A bare end aimed again leaves its node clear of the nodes around
        // and of the heads already on it.
        let free_ok = |which: usize, tip: Pt, next: Pt| {
            clear(tip, next)
                && edge
                    .ends
                    .get(which)
                    .is_none_or(|id| !scene.head_at(id, tip))
        };
        let checks = Checks {
            clear: &clear,
            free_ok: &free_ok,
        };
        let plan = candidates(&cur, &samples, h, &body, edge, (curve, gap), checks);
        let mut best: Option<(Pick, &Way)> = None;
        let mut trade: Option<Pick> = None;
        // The few best ways that graze, to push off what they graze.
        let mut grazing: Vec<(f64, Vec<Ob>, &Way)> = Vec::new();
        for (phase, ways) in plan.ways.iter().enumerate() {
            // Wide ways only when no close one clears the node cleanly.
            if best
                .as_ref()
                .is_some_and(|b| b.0.grazes.is_empty() && phase > 0)
            {
                break;
            }
            for (shift, way) in ways {
                if best.as_ref().is_some_and(|b| *shift >= b.0.score) {
                    break;
                }
                let Some(c) = plan.build(&cur, way, edge) else {
                    continue;
                };
                let bound = best.as_ref().map_or(f64::INFINITY, |b| b.0.score);
                match search.assess(&round, c, *shift, bound) {
                    Verdict::Clean(p) if best.as_ref().is_none_or(|b| p.score < b.0.score) => {
                        if !p.grazes.is_empty() {
                            grazing.push((p.score, p.grazes.clone(), way));
                        }
                        best = Some((p, way));
                    }
                    Verdict::Trade(p) if trade.as_ref().is_none_or(|t| p.score < t.score) => {
                        trade = Some(p);
                    }
                    _ => {}
                }
            }
        }
        // Ways that graze a node: push them off it (off each grazed node,
        // and off all together), a little more each time, until one clears.
        if best.as_ref().is_some_and(|b| !b.0.grazes.is_empty()) {
            grazing.sort_by(|a, b| a.0.total_cmp(&b.0));
            let pushed = PUSH.iter().find_map(|&k| {
                grazing
                    .iter()
                    .take(PUSHED)
                    .find_map(|(score, grazes, way)| {
                        let c = way.centre();
                        let off: Vec<Pt> = grazes
                            .iter()
                            .filter_map(|&o| scene.get(o).and_then(|n| unit(n.1.c, c)))
                            .collect();
                        let sum = off.iter().fold((0.0, 0.0), |v, u| (v.0 + u.0, v.1 + u.1));
                        let dirs = unit((0.0, 0.0), sum).into_iter().chain(off.iter().copied());
                        let base = score - GRAZE_COST * grazes.len() as f64;
                        dirs.into_iter().find_map(|u| {
                            let moved = way.shifted((u.0 * k, u.1 * k));
                            let c = plan.build(&cur, &moved, edge)?;
                            match search.assess(&round, c, base + k, f64::INFINITY) {
                                Verdict::Clean(p) if p.grazes.is_empty() => Some((p, *way)),
                                _ => None,
                            }
                        })
                    })
            });
            if pushed.is_some() {
                best = pushed;
            }
        }
        // A way that still grazes loses to one that clears this node and
        // leaves another for the next round, which may find a clean way.
        let clean = best.filter(|b| b.0.grazes.is_empty() || trade.is_none());
        match clean.map(|b| b.0).or(trade) {
            Some(p) => {
                passed.push(h.ob);
                hit = scene.hits(&p.samples, true, 0.0, owned);
                (cur, samples, moved) = (p.ctrl, p.samples, p.moved);
            }
            None => stuck.push(h.ob),
        }
    }
    let after = tally(scene, &samples, owned);
    (after.0 < before.0 || (after.0 == before.0 && after.1 < before.1)).then_some(cur)
}
