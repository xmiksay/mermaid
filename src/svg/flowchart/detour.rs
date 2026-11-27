//! Detours (#403): an edge whose drawn curve runs through a node it does not
//! end on is steered around that node by one waypoint, or a pair, beside it,
//! out in the gap between nodes, on the side the curve already leans toward.
//! A detour is kept only when the new curve clears that node and hits no node
//! or other edge's marker the old one did not; otherwise the other side, then
//! wider offsets, are tried. The waypoints go between the end runs (face
//! point, lead-in and the point beyond), so heads stay square-on and straight
//! under their markers; a round end is aimed again from its new neighbour.

use crate::parse::{EdgeCurve, NodeShape};
use crate::sugiyama::LayoutConfig;

use super::edges::{clip_end, EndClip};
use super::round::is_round;
use super::route::{EdgePath, Pt, LEAD_MARGIN};

mod body;
mod scene;

use body::{dist, dot, drawn, unit, Body, SAMPLES};
use scene::{Hit, Scene};

/// Detour rounds per edge, one node each.
const MAX_ROUNDS: usize = 6;
/// How far beyond the node a detour runs, in node gaps: mid-gap first.
const OFFSETS: [f64; 3] = [0.5, 1.0, 2.0];
/// The run kept straight off an unmarked end.
const BARE_RUN: f64 = 8.0;
/// The closest a detour waypoint comes to a neighbouring control point.
const MIN_SPACING: f64 = 10.0;

/// A node edges may not run through, unless they end on it.
pub(super) struct Obstacle<'a> {
    pub(super) id: &'a str,
    pub(super) node: EndClip,
}

/// What a detour needs of an edge: its end ids, the end shapes (a subgraph
/// end owns the nodes inside its frame) and lead-in lengths (0 for an
/// unmarked end).
pub(super) struct DetourEdge<'a> {
    pub(super) ends: [&'a str; 2],
    pub(super) clips: [EndClip; 2],
    pub(super) leads: [f64; 2],
}

impl DetourEdge<'_> {
    /// Whether the node `id` at `c` belongs to an end: is one, or sits
    /// inside a subgraph end's frame.
    fn owns(&self, id: &str, c: Pt) -> bool {
        self.ends.contains(&id)
            || self.clips.iter().any(|e| {
                e.shape.is_none()
                    && (c.0 - e.center.0).abs() <= e.size.0 / 2.0
                    && (c.1 - e.center.1).abs() <= e.size.1 / 2.0
            })
    }
}

/// Detour every edge (self-loops excepted) around the nodes it runs through.
/// Other edges' markers, where it does not end itself, are avoided too, but
/// only by a detour: an edge already running over one is not moved for it.
pub(super) fn detour_all(
    paths: &mut [Option<EdgePath>],
    edges: &[Option<DetourEdge>],
    obstacles: &[Obstacle],
) {
    let gap = LayoutConfig::default().node_gap;
    let nodes: Vec<(&str, Body)> = obstacles
        .iter()
        .filter_map(|o| Some((o.id, Body::new(&o.node)?)))
        .collect();
    let marks: Vec<(&str, Body)> = paths
        .iter()
        .zip(edges)
        .filter_map(|(p, e)| Some((p.as_ref()?, e.as_ref()?)))
        .flat_map(|(p, e)| {
            (0..2).filter_map(move |which| Some((e.ends[which], mark(p, e, which)?)))
        })
        .collect();
    let scene = Scene::new(nodes, marks);
    for (path, edge) in paths.iter_mut().zip(edges) {
        let (Some(path), Some(edge)) = (path, edge) else {
            continue;
        };
        if edge.ends[0] == edge.ends[1] {
            continue;
        }
        if let Some(ctrl) = detour(&path.ctrl, path.curve, edge, &scene, gap) {
            path.label = ctrl.clone();
            path.ctrl = ctrl;
        }
    }
}

/// The marker at end `which` of an edge as a disc over its length, with a
/// margin.
fn mark(path: &EdgePath, edge: &DetourEdge, which: usize) -> Option<Body> {
    let len = edge.leads[which] - LEAD_MARGIN;
    if len <= 0.0 {
        return None;
    }
    let n = path.ctrl.len();
    let (tip, next) = if which == 0 {
        (*path.ctrl.first()?, *path.ctrl.get(1)?)
    } else {
        (*path.ctrl.last()?, *path.ctrl.get(n.checked_sub(2)?)?)
    };
    let u = unit(tip, next)?;
    Body::new(&EndClip {
        center: (tip.0 + u.0 * len / 2.0, tip.1 + u.1 * len / 2.0),
        size: (len + 2.0 * LEAD_MARGIN, len + 2.0 * LEAD_MARGIN),
        shape: Some(NodeShape::Circle),
    })
}

/// The detoured control points, `None` when the curve hits nothing or no
/// detour helps. Each round takes the first node still hit (markers never
/// start a round) and keeps the first candidate that clears it without
/// hitting any node or marker the curve did not hit already.
fn detour(
    ctrl: &[Pt],
    curve: EdgeCurve,
    edge: &DetourEdge,
    scene: &Scene,
    gap: f64,
) -> Option<Vec<Pt>> {
    let owned = |ob: usize| match (scene.ids.get(ob), scene.bodies.get(ob)) {
        (Some(id), Some(b)) => edge.owns(id, b.c),
        _ => true,
    };
    let mut samples = drawn(ctrl, curve);
    // Markers matter only once a node is hit: most edges never get this far.
    if scene.hits(&samples, false, owned).is_empty() {
        return None;
    }
    let mut hit = scene.hits(&samples, true, owned);
    let mut cur = ctrl.to_vec();
    let mut stuck: Vec<usize> = Vec::new();
    let mut changed = false;
    for _ in 0..MAX_ROUNDS {
        let Some(h) = hit
            .iter()
            .find(|h| h.ob < scene.nodes && !stuck.contains(&h.ob))
        else {
            break;
        };
        let Some(body) = scene.bodies.get(h.ob) else {
            break;
        };
        let rest: Vec<usize> = hit.iter().map(|x| x.ob).filter(|&o| o != h.ob).collect();
        let found = candidates(&cur, &samples, h, body, edge, gap)
            .into_iter()
            .find_map(|c| {
                let s = drawn(&c, curve);
                let nh = scene.hits(&s, true, owned);
                nh.iter()
                    .all(|x| rest.contains(&x.ob))
                    .then_some((c, s, nh))
            });
        match found {
            Some((c, s, nh)) => {
                (cur, samples, hit) = (c, s, nh);
                changed = true;
            }
            None => stuck.push(h.ob),
        }
    }
    changed.then_some(cur)
}

/// Detoured control points to try around the hit node, shorter side first,
/// then wider offsets.
fn candidates(
    ctrl: &[Pt],
    samples: &[Pt],
    h: &Hit,
    body: &Body,
    edge: &DetourEdge,
    gap: f64,
) -> Vec<Vec<Pt>> {
    // The travel direction over about one bezier either side of the hit.
    let entry = samples.get(h.first.saturating_sub(SAMPLES / 2));
    let exit = samples.get(h.last + 1 + SAMPLES / 2).or(samples.last());
    let (Some(&entry), Some(&exit)) = (entry, exit) else {
        return Vec::new();
    };
    let Some(d) = unit(entry, exit) else {
        return Vec::new();
    };
    let round = edge.clips.map(|c| c.shape.is_some_and(is_round));
    let Some((base, lo, hi)) = room(ctrl, edge.leads, round) else {
        return Vec::new();
    };
    let n = (-d.1, d.0);
    let mid = (
        (entry.0 + exit.0) / 2.0 - body.c.0,
        (entry.1 + exit.1) / 2.0 - body.c.1,
    );
    let s0 = if dot(mid, n) >= 0.0 { 1.0 } else { -1.0 };
    let (ext_n, ext_d) = (body.support(n), body.support(d));
    let mut out = Vec::new();
    for k in OFFSETS {
        for s in [s0, -s0] {
            let off = s * (ext_n + k * gap);
            let side = (body.c.0 + n.0 * off, body.c.1 + n.1 * off);
            let along = ext_d + k * gap / 2.0;
            let tries = [true, false]
                .into_iter()
                .flat_map(|single| insert(&base, (lo, hi), side, d, along, single));
            for mut c in tries {
                for (which, (node, lead)) in edge.clips.iter().zip(edge.leads).enumerate() {
                    if round[which] {
                        reaim(&mut c, which, node, lead);
                    }
                }
                out.push(c);
            }
        }
    }
    out
}

/// `ctrl` with each end's straight run marked off, and the range of segments
/// detour waypoints may go into: between the two runs. A round end's run is
/// dropped down to its tip: `reaim` rebuilds it toward the detour.
fn room(ctrl: &[Pt], leads: [f64; 2], round: [bool; 2]) -> Option<(Vec<Pt>, usize, usize)> {
    let n = ctrl.len();
    let rev: Vec<Pt> = ctrl.iter().rev().copied().collect();
    let (ks, xs, rs) = end_run(ctrl, leads[0])?;
    let (kt, xt, rt) = end_run(&rev, leads[1])?;
    if ks + kt + 2 > n || (n == 2 && rs + rt + 2.0 >= dist(ctrl[0], ctrl[1])) {
        return None;
    }
    let mut out: Vec<Pt> = ctrl[..1].to_vec();
    if !round[0] {
        out.extend(&ctrl[1..=ks]);
        out.extend(&xs);
    }
    let lo = out.len() - 1;
    out.extend(&ctrl[ks + 1..n - 1 - kt]);
    if !round[1] {
        out.extend(xt.iter().rev());
        out.extend(&ctrl[n - 1 - kt..n - 1]);
    }
    out.extend(&ctrl[n - 1..]);
    let tail = if round[1] { 0 } else { kt + xt.len() };
    let hi = (out.len() - tail).checked_sub(2)?;
    (lo <= hi).then_some((out, lo, hi))
}

/// Clip round end `which` again on the ray from its new neighbour toward
/// the centre, as `route` does, with its straight run (one and two lead-ins
/// on a marked end) on that ray.
fn reaim(ctrl: &mut Vec<Pt>, which: usize, node: &EndClip, lead: f64) {
    if which == 1 {
        ctrl.reverse();
    }
    if let Some(&next) = ctrl.get(1) {
        let tip = clip_end(next, node);
        let run: Vec<Pt> = match unit(tip, next) {
            Some(u) if lead > 0.0 => [lead, 2.0 * lead]
                .into_iter()
                .filter(|&t| t + 1.0 < dist(tip, next))
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

/// `ctrl` with the detour run beside the node — through `side`, ±`along`
/// on the travel direction `d` — put between two points `i < j` in
/// `lo..=hi + 1` that head along `d` past the node, in place of the (up to
/// `MAX_DROP`) points between them, clamped to their stretch along `d` so
/// the curve never runs backwards. Shortest result first, then fewest
/// points dropped; at most `KEEP` of them.
fn insert(
    ctrl: &[Pt],
    range: (usize, usize),
    side: Pt,
    d: Pt,
    along: f64,
    single: bool,
) -> Vec<Vec<Pt>> {
    let (lo, hi) = range;
    const MAX_DROP: usize = 1;
    const KEEP: usize = 2;
    let at = |t: f64| (side.0 + d.0 * t, side.1 + d.1 * t);
    let proj = |p: Pt| dot((p.0 - side.0, p.1 - side.1), d);
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
            let new_len: f64 = std::iter::once(p)
                .chain(run.iter().copied())
                .chain(std::iter::once(q))
                .collect::<Vec<_>>()
                .windows(2)
                .map(|w| dist(w[0], w[1]))
                .sum();
            let old_len: f64 = old.windows(2).map(|w| dist(w[0], w[1])).sum();
            let cost = new_len - old_len;
            if cost.is_finite() {
                let mut out = ctrl[..=i].to_vec();
                out.extend(run);
                out.extend(&ctrl[j..]);
                found.push((cost, j - i, i, out));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    found.into_iter().take(KEEP).map(|f| f.3).collect()
}

#[cfg(test)]
mod tests;
