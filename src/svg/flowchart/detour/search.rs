//! The search for a detour: round by round, the first node the curve still
//! runs through, every way round it scored, the best one kept.

use crate::parse::EdgeCurve;

use super::super::route::Pt;
use super::body::{drawn, unit};
use super::cands::{candidates, Checks, Way};
use super::judge::{
    measure, settle, Best, Pick, Round, Search, Verdict, GRAZE, GRAZE_COST, NEAR_ANY,
};
use super::scene::{bbox, Ob, Scene, CLOSE};
use super::DetourEdge;

/// Detour rounds per edge, one node each.
const MAX_ROUNDS: usize = 10;
/// How far a way that grazes a node is pushed off it, in turn.
const PUSH: [f64; 7] = [3.0, 6.0, 10.0, 15.0, 22.0, 32.0, 45.0];
/// How many grazing ways are pushed.
const PUSHED: usize = 3;

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
    let mut samples = drawn(ctrl, curve, &|b| scene.near_any(b, NEAR_ANY));
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
        let mut trade: Option<Pick> = None;
        // The best clean way, and the few that graze, to push off what they
        // graze; clean ways wait to be scored in full until they may win.
        let mut found: Best<Way> = (None, Vec::new());
        let mut pending: Vec<(Pick, &Way)> = Vec::new();
        for (phase, ways) in plan.ways.iter().enumerate() {
            // Wide ways only when no close one clears the node cleanly.
            if found
                .0
                .as_ref()
                .is_some_and(|b| b.0.grazes.is_empty() && phase > 0)
            {
                break;
            }
            for (shift, way) in ways {
                settle(&mut pending, &mut found, *shift, along);
                let bound = found.0.as_ref().map_or(f64::INFINITY, |b| b.0.score);
                if *shift >= bound {
                    break;
                }
                let Some(c) = plan.build(&cur, way, edge) else {
                    continue;
                };
                match search.assess(&round, c, *shift, bound) {
                    Verdict::Clean(p) => pending.push((p, way)),
                    Verdict::Trade(p) if trade.as_ref().is_none_or(|t| p.score < t.score) => {
                        trade = Some(p);
                    }
                    _ => {}
                }
            }
            settle(&mut pending, &mut found, f64::INFINITY, along);
        }
        let (mut best, mut grazing) = found;
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
