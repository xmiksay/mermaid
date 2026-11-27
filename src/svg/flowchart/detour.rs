//! Detours (#403): an edge whose drawn curve runs through a node it does not
//! end on is steered around that node by one waypoint, or a pair, beside the
//! stretch it crosses, out in the gap between nodes. Every candidate that
//! clears the node without hitting another node or marker, and without
//! running backwards or doubling the edge, is scored: how far it moves the
//! curve aside (so the shorter side wins by default), plus heavy penalties
//! for running along another edge or a frame border and for grazing a node.
//! The waypoints go between the end runs (face point, lead-in and the point
//! beyond), so heads stay square-on and straight under their markers; a
//! round end is aimed again from its new neighbour.

use std::cell::RefCell;

use crate::parse::{EdgeCurve, NodeShape};
use crate::sugiyama::LayoutConfig;

use super::edges::EndClip;
use super::route::{EdgePath, Pt, LEAD_MARGIN};

mod body;
mod cands;
mod index;
mod lines;
mod room;
mod scene;

use body::{dist, drawn, unit, Body, SAMPLES};
use cands::candidates;
use lines::Lines;
use scene::Scene;

/// Detour rounds per edge, one node each.
const MAX_ROUNDS: usize = 6;
/// A curve this close to a node grazes it.
const GRAZE: f64 = 6.0;
/// Score per new node grazed (basis curves; others may not graze at all).
const GRAZE_COST: f64 = 40.0;
/// Score per other edge's marker newly run over.
const MARK_COST: f64 = 150.0;
/// Score per px of running along another line (for stretches past `RUN`).
const ALONG_COST: f64 = 4.0;
/// Slack on how far a detour may run back against the edge's direction.
const BACKTRACK: f64 = 10.0;

/// A node edges may not run through, unless they end on it.
pub(super) struct Obstacle<'a> {
    pub(super) id: &'a str,
    pub(super) node: EndClip,
}

/// What a detour needs of an edge: its end ids, the end shapes (a subgraph
/// end may own the nodes inside its frame) and lead-in lengths (0 for an
/// unmarked end).
pub(super) struct DetourEdge<'a> {
    pub(super) ends: [&'a str; 2],
    pub(super) clips: [EndClip; 2],
    pub(super) leads: [f64; 2],
    /// The flow axis between the ends (0 x, 1 y).
    pub(super) flow: usize,
}

/// Whether `c` lies within the box of `e`.
fn within(e: &EndClip, c: Pt) -> bool {
    (c.0 - e.center.0).abs() <= e.size.0 / 2.0 && (c.1 - e.center.1).abs() <= e.size.1 / 2.0
}

impl DetourEdge<'_> {
    /// Whether the node `id` at `c` belongs to an end: is one, or sits
    /// inside a subgraph end's frame that the other end lies inside too (a
    /// member to its own frame, or nested frames) — an edge between a
    /// frame and the outside still keeps off its members.
    fn owns(&self, id: &str, c: Pt) -> bool {
        let [a, b] = &self.clips;
        self.ends.contains(&id)
            || [(a, b), (b, a)]
                .iter()
                .any(|(e, other)| e.shape.is_none() && within(e, other.center) && within(e, c))
    }
}

/// Detour every edge (self-loops excepted) around the nodes it runs through.
/// Other edges' markers, where it does not end itself, are avoided too, but
/// only by a detour: an edge already running over one is not moved for it.
/// `frames` are the subgraph frames `(x0, y0, x1, y1)`.
pub(super) fn detour_all(
    paths: &mut [Option<EdgePath>],
    edges: &[Option<DetourEdge>],
    obstacles: &[Obstacle],
    frames: &[[f64; 4]],
) {
    let gap = LayoutConfig::default().node_gap;
    let nodes: Vec<(&str, Body)> = obstacles
        .iter()
        .filter_map(|o| Some((o.id, Body::new(&o.node)?)))
        .collect();
    let first_marks = paths
        .iter()
        .zip(edges)
        .enumerate()
        .filter_map(|(i, (p, e))| Some((i, p.as_ref()?, e.as_ref()?)))
        .flat_map(|(i, p, e)| marks(&p.ctrl, e).into_iter().map(move |m| (i, m)))
        .collect();
    let mut scene = Scene::new(nodes, first_marks);
    // Each edge's drawn points, sampled when a detour nearby first needs them.
    let drawn_cache: RefCell<Vec<Option<Vec<Pt>>>> = RefCell::new(vec![None; paths.len()]);
    for i in 0..paths.len() {
        let (Some(Some(path)), Some(Some(edge))) = (paths.get(i), edges.get(i)) else {
            continue;
        };
        if edge.ends[0] == edge.ends[1] {
            continue;
        }
        let lines = |region: [f64; 4]| lines_near(paths, &drawn_cache, frames, i, region);
        let Some(ctrl) = detour(&path.ctrl, path.curve, edge, &scene, gap, lines) else {
            continue;
        };
        scene.moved(i, marks(&ctrl, edge));
        if let Some(Some(path)) = paths.get_mut(i) {
            path.label = ctrl.clone();
            path.ctrl = ctrl;
            if let Some(slot) = drawn_cache.borrow_mut().get_mut(i) {
                *slot = None;
            }
        }
    }
}

/// The drawn lines of every edge but `skip`, and the frame borders, that
/// overlap `region`.
fn lines_near(
    paths: &[Option<EdgePath>],
    cache: &RefCell<Vec<Option<Vec<Pt>>>>,
    frames: &[[f64; 4]],
    skip: usize,
    region: [f64; 4],
) -> Lines {
    let mut cache = cache.borrow_mut();
    let meets = |b: [f64; 4]| {
        b[0] <= region[2] && b[2] >= region[0] && b[1] <= region[3] && b[3] >= region[1]
    };
    let mut segs: Vec<(Pt, Pt)> = Vec::new();
    for (j, p) in paths.iter().enumerate() {
        let Some(p) = p.as_ref().filter(|_| j != skip) else {
            continue;
        };
        if !meets(hull(&p.ctrl)) {
            continue;
        }
        let Some(slot) = cache.get_mut(j) else {
            continue;
        };
        let s = slot.get_or_insert_with(|| p.drawn_points());
        segs.extend(
            s.windows(2)
                .map(|w| (w[0], w[1]))
                .filter(|&(a, b)| meets(hull(&[a, b]))),
        );
    }
    for f in frames.iter().filter(|f| meets(**f)) {
        let c = [(f[0], f[1]), (f[2], f[1]), (f[2], f[3]), (f[0], f[3])];
        segs.extend(
            c.iter()
                .zip(c.iter().cycle().skip(1))
                .map(|(a, b)| (*a, *b)),
        );
    }
    Lines::new(segs, region)
}

/// The bounding box of `pts`; a basis curve stays inside its control
/// points' (so do linear and step ones).
fn hull(pts: &[Pt]) -> [f64; 4] {
    let inf = f64::INFINITY;
    pts.iter().fold([inf, inf, -inf, -inf], |m, p| {
        [m[0].min(p.0), m[1].min(p.1), m[2].max(p.0), m[3].max(p.1)]
    })
}

/// The markers of an edge drawn through `ctrl`.
fn marks<'a>(ctrl: &[Pt], edge: &DetourEdge<'a>) -> Vec<(&'a str, Body)> {
    (0..2)
        .filter_map(|which| Some((edge.ends[which], mark(ctrl, edge, which)?)))
        .collect()
}

/// The marker at end `which` of an edge as a disc over its length, with a
/// margin.
fn mark(ctrl: &[Pt], edge: &DetourEdge, which: usize) -> Option<Body> {
    let len = edge.leads[which] - LEAD_MARGIN;
    if len <= 0.0 {
        return None;
    }
    let n = ctrl.len();
    let (tip, next) = if which == 0 {
        (*ctrl.first()?, *ctrl.get(1)?)
    } else {
        (*ctrl.last()?, *ctrl.get(n.checked_sub(2)?)?)
    };
    let u = unit(tip, next)?;
    Body::new(&EndClip {
        center: (tip.0 + u.0 * len / 2.0, tip.1 + u.1 * len / 2.0),
        size: (len + 2.0 * LEAD_MARGIN, len + 2.0 * LEAD_MARGIN),
        shape: Some(NodeShape::Circle),
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

/// A candidate kept: score, control points, samples, length and backtrack.
type Pick = (f64, Vec<Pt>, Vec<Pt>, (f64, f64));

/// Length and backtrack of a sampled curve: travel against the direction
/// the edge heads along the flow axis `flow`, from tip to tip (`tips`; along
/// their line when both sit in one rank). Going round the end of a wide
/// node may run back across the flow, never against it.
fn measure(samples: &[Pt], flow: usize, (a, b): (Pt, Pt)) -> (f64, f64) {
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

/// The detoured control points, `None` when the curve hits nothing or no
/// detour helps. Each round takes the first node still hit (markers never
/// start a round) and keeps the best-scoring candidate that clears it
/// without hitting any node or marker the curve did not hit already.
fn detour(
    ctrl: &[Pt],
    curve: EdgeCurve,
    edge: &DetourEdge,
    scene: &Scene,
    gap: f64,
    lines: impl Fn([f64; 4]) -> Lines,
) -> Option<Vec<Pt>> {
    let owned = |id: &str, c: Pt| edge.owns(id, c);
    if !scene.may_hit(&hull(ctrl), owned) {
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
    let (mut len, mut back) = (len0, back0);
    let strict = curve != EdgeCurve::Basis;
    let mut hit = scene.hits(&samples, true, 0.0, owned);
    let mut cur = ctrl.to_vec();
    let mut stuck = Vec::new();
    let mut passed = Vec::new();
    let mut changed = false;
    for _ in 0..MAX_ROUNDS {
        let Some(h) = hit.iter().find(|h| h.ob.0 && !stuck.contains(&h.ob)) else {
            break;
        };
        let Some(&(_, body)) = scene.get(h.ob) else {
            break;
        };
        let rest: Vec<_> = hit.iter().map(|x| x.ob).filter(|&o| o != h.ob).collect();
        let grazed: Vec<_> = scene
            .hits(&samples, false, GRAZE, owned)
            .iter()
            .map(|x| x.ob)
            .filter(|&o| o != h.ob)
            .collect();
        let mut best: Option<Pick> = None;
        let mut trade: Option<Pick> = None;
        let mut near: Option<Lines> = None;
        let plan = candidates(&cur, &samples, h, &body, edge, curve, gap);
        let ways = plan
            .ways
            .iter()
            .enumerate()
            .flat_map(|(phase, w)| w.iter().map(move |(shift, way)| (phase, *shift, way)));
        for (phase, shift, way) in ways {
            // A wide way is tried only when no close one works.
            if best.as_ref().is_some_and(|b| shift >= b.0 || phase > 0) {
                break;
            }
            let Some(ctrl) = plan.build(&cur, way, edge) else {
                continue;
            };
            let s = drawn(&ctrl, curve);
            // Only the stretch that changed can hit anything new; the node
            // being passed must not be hit in what did not change.
            let (pre, post) = shared(&samples, &s);
            let kept = samples.len().saturating_sub(post);
            if h.first + 1 < pre || h.last >= kept {
                continue;
            }
            let span = |v: &[Pt]| pre.saturating_sub(1)..v.len() + 1 - post.max(1);
            let (Some(part), Some(was)) = (s.get(span(&s)), samples.get(span(&samples))) else {
                continue;
            };
            let ((nl, nb), (ol, ob)) = (
                measure(part, edge.flow, tips),
                measure(was, edge.flow, tips),
            );
            let moved = (len + nl - ol, back + nb - ob);
            if moved.0 > 2.0 * len0 + gap || moved.1 > back0 + BACKTRACK {
                continue;
            }
            let touch = scene.touches(part, GRAZE, owned);
            let fresh: Vec<_> = touch.cut.iter().filter(|o| !rest.contains(o)).collect();
            if let [&one] = fresh[..] {
                // Trading this node for one never passed before is progress
                // the next round can finish, if nothing clean turns up.
                if !passed.contains(&one) && trade.as_ref().is_none_or(|t| shift < t.0) {
                    trade = Some((shift, ctrl, s, moved));
                }
                continue;
            }
            let new_grazes = touch.grazed.iter().filter(|o| !grazed.contains(o)).count();
            if !fresh.is_empty() || (strict && new_grazes > 0) {
                continue;
            }
            let new_marks = touch.marks.iter().filter(|o| !rest.contains(o)).count();
            let base = shift + GRAZE_COST * new_grazes as f64 + MARK_COST * new_marks as f64;
            if best.as_ref().is_some_and(|b| base >= b.0) {
                continue;
            }
            // Candidates differ only within a few gaps of the node: what
            // they run along further off is the same for all of them.
            let near = near.get_or_insert_with(|| {
                let (r, m) = (body.bbox, 3.0 * gap);
                lines([r[0] - m, r[1] - m, r[2] + m, r[3] + m])
            });
            let pad = |k: usize| k.saturating_sub(SAMPLES);
            let around = s.get(pad(pre)..s.len() - pad(post)).unwrap_or(&s);
            let along = near.along(around);
            let score = base + ALONG_COST * along;
            if best.as_ref().is_none_or(|b| score < b.0) {
                best = Some((score, ctrl, s, moved));
            }
        }
        match best.or(trade) {
            Some((_, c, s, (l, b))) => {
                passed.push(h.ob);
                hit = scene.hits(&s, true, 0.0, owned);
                (cur, samples) = (c, s);
                (len, back) = (l, b);
                changed = true;
            }
            None => stuck.push(h.ob),
        }
    }
    changed.then_some(cur)
}

#[cfg(test)]
mod tests;
