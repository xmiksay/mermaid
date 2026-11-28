//! The search for a detour: round by round, the first node the curve still
//! runs through, every way round it scored, the best one kept.

use crate::parse::EdgeCurve;

use super::super::route::Pt;
use super::body::{dist, drawn, unit, SAMPLES};
use super::cands::{candidates, Checks, Way};
use super::scene::{bbox, Hit, Ob, Scene, CLOSE};
use super::DetourEdge;

/// Detour rounds per edge, one node each.
const MAX_ROUNDS: usize = 10;
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
const PUSH: [f64; 7] = [3.0, 6.0, 10.0, 15.0, 22.0, 32.0, 45.0];
/// How many grazing ways are pushed.
const PUSHED: usize = 3;

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
    /// Nodes stacked on an end, which the edge may run through.
    stacked: &'s [&'s str],
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
    /// How long a sampled stretch runs along other edges or frame borders.
    along: &'r dyn Fn(&[Pt]) -> f64,
    /// The round takes the curve off a node it comes close to, not out of
    /// one it runs through.
    off: bool,
}

impl Search<'_, '_, '_> {
    fn owned(&self) -> impl Fn(&str, Pt) -> bool + '_ {
        |id, c| self.edge.owns(id, c) || self.stacked.contains(&id)
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
            (r.along)(s.get(pad(pre)..s.len() - pad(post)).unwrap_or(&s))
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

/// How many nodes the sampled curve runs through, comes `CLOSE` to and
/// grazes (nodes it runs through counted once).
fn tally(scene: &Scene, samples: &[Pt], owned: impl Fn(&str, Pt) -> bool) -> (usize, usize, usize) {
    let t = scene.touches(samples, GRAZE, owned);
    let near = |v: &[Ob]| v.iter().filter(|o| !t.cut.contains(o)).count();
    (t.cut.len(), near(&t.close), near(&t.grazed))
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
    along: &dyn Fn(&[Pt]) -> f64,
) -> Option<Vec<Pt>> {
    let stacked = scene.stacked(edge);
    let owned = |id: &str, c: Pt| edge.owns(id, c) || stacked.contains(&id);
    if !scene.may_hit(&bbox(ctrl), owned) {
        return None;
    }
    // Sampled as finely as its longest span needs, kept for every candidate
    // so the stretch a candidate shares with it compares sample for sample.
    let mut samples = drawn(ctrl, curve);
    if !scene.hits_any(&samples, false, 0.0, owned) {
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
        stacked: &stacked,
        len0,
        back0,
    };
    let before = tally(scene, &samples, owned);
    let mut moved = (len0, back0);
    let mut hit = scene.hits(&samples, true, 0.0, owned);
    let mut cur = ctrl.to_vec();
    let (mut stuck, mut passed) = (Vec::new(), Vec::new());
    let mut keep: Vec<((usize, usize, usize), Vec<Pt>)> = Vec::new();
    let mut aside: Option<Pick> = None;
    // Nodes the input already comes close to (and does not run through).
    let close0: Vec<Ob> = {
        let t = scene.touches(&samples, GRAZE, owned);
        t.close.into_iter().filter(|o| !t.cut.contains(o)).collect()
    };
    for _ in 0..MAX_ROUNDS {
        // Through a node first; then, once through none, close to a node the
        // input was not: a round takes the curve off that one too.
        let cut = hit.iter().find(|h| h.ob.0 && !stuck.contains(&h.ob));
        let near = scene
            .hits(&samples, false, CLOSE, owned)
            .into_iter()
            .find(|h| !close0.contains(&h.ob) && !stuck.contains(&h.ob));
        let (h, off) = match (cut, &near) {
            (Some(h), _) => (h, false),
            (None, Some(h)) if hit.iter().all(|x| !x.ob.0) => (h, true),
            _ => break,
        };
        let Some(&(_, body)) = scene.get(h.ob) else {
            break;
        };
        let body = if off { body.grown(CLOSE) } else { body };
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
            along,
            off,
        };
        let clear = |a: Pt, b: Pt| !scene.hits_any(&[a, b], false, GRAZE, owned);
        // A bare end aimed again leaves its node clear of the nodes around
        // and of the heads already on it.
        let free_ok = |which: usize, tip: Pt, next: Pt| {
            clear(tip, next)
                && edge
                    .ends
                    .get(which)
                    .is_none_or(|id| !scene.head_at(id, tip, edge.index))
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
                for c in plan.build(&cur, way, edge) {
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
                            plan.build(&cur, &moved, edge).into_iter().find_map(|c| {
                                match search.assess(&round, c, base + k, f64::INFINITY) {
                                    Verdict::Clean(p) if p.grazes.is_empty() => Some((p, *way)),
                                    _ => None,
                                }
                            })
                        })
                    })
            });
            if pushed.is_some() {
                best = pushed;
            }
        }
        // A way that still grazes loses to one that clears this node and
        // leaves another for the next round, which may find a clean way; it
        // is set aside, to go on from if that runs stuck.
        let (clean, later) = match best {
            Some(b) if !b.0.grazes.is_empty() && trade.is_some() => (None, Some(b.0)),
            b => (b, None),
        };
        if let Some(p) = later {
            keep.push((tally(scene, &p.samples, owned), p.ctrl.clone()));
            aside = Some(p);
        }
        match clean.map(|b| b.0).or(trade).or_else(|| aside.take()) {
            Some(p) => {
                passed.push(h.ob);
                hit = scene.hits(&p.samples, true, 0.0, owned);
                (cur, samples, moved) = (p.ctrl, p.samples, p.moved);
                keep.push((tally(scene, &samples, owned), cur.clone()));
            }
            None => stuck.push(h.ob),
        }
    }
    // The best curve any round reached: through fewest nodes, then coming
    // close to fewest, then grazing fewest; only if better than the input.
    keep.sort_by_key(|k| k.0);
    keep.into_iter()
        .next()
        .filter(|k| k.0 < before)
        .map(|k| k.1)
}
