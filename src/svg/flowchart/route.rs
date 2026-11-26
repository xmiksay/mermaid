//! Flowchart edge geometry: the control points an edge is drawn through, built
//! in centre space and clipped to the end shapes last. Shared by the
//! canvas-measuring pass and the drawing pass.

use crate::parse::{EdgeCurve, EdgeHead, EdgeLine, FlowEdge};
use crate::sugiyama::LayoutConfig;
use crate::svg::builder::curve_basis_points;
use crate::svg::geometry::clip_polyline;
use crate::svg::style::ResolvedStyle;

use super::edges::{basis_control_points, clip_end, EdgeGeometry, EndClip};
use super::round::{clear_of_nodes, is_round, round_cross};
use super::slots::slot_offsets;

pub(super) type Pt = (f64, f64);

/// Below this flow-axis offset two ends sit in one rank.
pub(super) const ALIGNED_EPS: f64 = 2.0;
/// Clearance between an arrowhead's base and the point where the curve may
/// start to turn.
const LEAD_MARGIN: f64 = 4.0;

/// One edge, resolved: what is drawn and what its label is placed along.
pub(super) struct EdgePath {
    /// Control points handed to the curve builder.
    pub(super) ctrl: Vec<Pt>,
    /// Polyline the label is placed along.
    pub(super) label: Vec<Pt>,
    pub(super) curve: EdgeCurve,
    pub(super) style: ResolvedStyle,
}

impl EdgePath {
    pub(super) fn shift(&mut self, dx: f64, dy: f64) {
        for p in self.ctrl.iter_mut().chain(self.label.iter_mut()) {
            p.0 += dx;
            p.1 += dy;
        }
    }

    /// Points on the drawn curve (sampled for basis), what the canvas
    /// measures so only a real overflow moves it.
    pub(super) fn drawn_points(&self) -> Vec<Pt> {
        match self.curve {
            EdgeCurve::Linear | EdgeCurve::Step => self.ctrl.clone(),
            _ => curve_basis_points(&self.ctrl),
        }
    }
}

/// One edge as the router needs it. `paired` marks a self-loop or one half of
/// an opposite pair: those, cluster ends and linear/step curves keep the plain
/// clip-first route.
pub(super) struct EdgeIn<'a> {
    pub(super) geom: EdgeGeometry,
    pub(super) edge: &'a FlowEdge,
    pub(super) curve: EdgeCurve,
    pub(super) style: ResolvedStyle,
    pub(super) horizontal: bool,
    pub(super) paired: bool,
}

impl EdgeIn<'_> {
    fn plain(&self) -> bool {
        let (start, end, _) = &self.geom;
        self.paired
            || start.shape.is_none()
            || end.shape.is_none()
            || matches!(self.curve, EdgeCurve::Linear | EdgeCurve::Step)
            || self.skips_ranks()
    }

    /// A two-point edge spanning more than one rank gap: the layout never
    /// routed it (a local `direction` replaces a cross-lane route with a bare
    /// line), so an S through the ranks it skips would run through their
    /// nodes.
    fn skips_ranks(&self) -> bool {
        let (start, end, pts) = &self.geom;
        let f = self.axes().0;
        let half = |n: &EndClip| axes(n.size)[f] / 2.0;
        let gap = (end.center_on(f) - start.center_on(f)).abs() - half(start) - half(end);
        pts.len() == 2 && gap > 2.0 * LayoutConfig::default().layer_gap
    }

    /// The flow and cross axis indices.
    pub(super) fn axes(&self) -> (usize, usize) {
        if self.horizontal {
            (0, 1)
        } else {
            (1, 0)
        }
    }

    /// The marker at end `which` (0 start, 1 end).
    pub(super) fn head(&self, which: usize) -> EdgeHead {
        [self.edge.tail, self.edge.head][which]
    }

    pub(super) fn node(&self, which: usize) -> &EndClip {
        if which == 0 {
            &self.geom.0
        } else {
            &self.geom.1
        }
    }

    fn lead(&self, which: usize) -> f64 {
        marker_reach(self.head(which)).map_or(0.0, |r| r * self.stroke() + LEAD_MARGIN)
    }

    /// The stroke width: a usable `linkStyle` override, else the line kind's.
    pub(super) fn stroke(&self) -> f64 {
        let default = if self.edge.line == EdgeLine::Thick {
            3.0
        } else {
            1.5
        };
        self.style
            .stroke_width
            .as_deref()
            .and_then(|w| w.trim().parse::<f64>().ok())
            .filter(|w| w.is_finite() && *w >= 0.0)
            .unwrap_or(default)
    }
}

/// One end's square-on approach point, before slotting: its depth on the flow
/// axis `f`, the face it enters (`side`: the face toward +`f`), and the cross
/// coordinate its edge comes from (orders the slots on a face).
pub(super) struct Approach {
    pub(super) f: usize,
    pub(super) depth: f64,
    pub(super) side: bool,
    pub(super) from_cross: f64,
}

/// Route every edge; `None` stays `None` (invisible or unresolved).
/// `nodes` are every node's `(centre, size)`, the obstacles a round end's
/// approach must not cross.
pub(super) fn route_all(ins: Vec<Option<EdgeIn>>, nodes: &[(Pt, Pt)]) -> Vec<Option<EdgePath>> {
    let aps: Vec<[Option<Approach>; 2]> = ins
        .iter()
        .map(|e| {
            e.as_ref()
                .map_or([None, None], |e| clear_of_nodes(e, approaches(e), nodes))
        })
        .collect();
    let offsets = slot_offsets(&ins, &aps);
    ins.into_iter()
        .zip(aps.iter().zip(offsets))
        .map(|(e, (ap, off))| e.map(|e| route_edge(e, ap, off)))
        .collect()
}

/// Each end's approach point. Ends in different ranks (forward or back
/// edges) approach across the flow axis, so they meet their flow face
/// square-on however wide the boxes are; the axis comes from the rank
/// relation, never from the geometry. Ends in one rank join straight.
fn approaches(e: &EdgeIn) -> [Option<Approach>; 2] {
    if e.plain() {
        return [None, None];
    }
    let (start, end, pts) = &e.geom;
    let (f, c) = e.axes();
    let leads = (e.lead(0), e.lead(1));
    let n = pts.len();
    if n > 2 {
        return [
            end_approach(start, pts[1], f, c, leads.0),
            end_approach(end, pts[n - 2], f, c, leads.1),
        ];
    }
    let Some(mid) = pair_depth(start, end, f, leads) else {
        return [None, None];
    };
    let dir = end.center_on(f) > start.center_on(f);
    [
        Some(Approach {
            f,
            depth: mid,
            side: dir,
            from_cross: end.center_on(c),
        }),
        Some(Approach {
            f,
            depth: mid,
            side: !dir,
            from_cross: start.center_on(c),
        }),
    ]
}

/// The flow-axis depth of a two-point edge's S bend: mid-way through the gap
/// between the facing faces, moved toward an unmarked end when that leaves
/// each marked end room for its straight lead-in (`leads`). `None` for ends in
/// one rank or a gap too tight for both lead-ins: those draw straight, aimed
/// at the centres, which is straight under any marker.
fn pair_depth(start: &EndClip, end: &EndClip, f: usize, leads: (f64, f64)) -> Option<f64> {
    let (s, t) = (start.center_on(f), end.center_on(f));
    if (t - s).abs() <= ALIGNED_EPS {
        return None;
    }
    let dir = (t - s).signum();
    let lo = s + dir * axes(start.size)[f] / 2.0;
    let gap = (t - dir * axes(end.size)[f] / 2.0 - lo) * dir;
    let (min_s, min_t) = (leads.0 + 2.0, leads.1 + 2.0);
    (gap >= min_s + min_t).then(|| lo + dir * (gap / 2.0).max(min_s).min(gap - min_t))
}

/// A routed end's approach point, square off its face just past the lead-in.
/// Kept that shallow (not mid-way to the waypoint) because a routed edge's
/// last waypoint can sit far off to the side: a deep square run then swings
/// the curve through neighbouring nodes. `None` when the waypoint is closer,
/// and for an unmarked end: with no head to square up it keeps the ray from
/// its waypoint, which leaves the node on the side it is heading for.
fn end_approach(node: &EndClip, toward: Pt, f: usize, c: usize, lead: f64) -> Option<Approach> {
    if lead <= 0.0 {
        return None;
    }
    let (o, a) = (node.center_on(f), axes(toward)[f]);
    let dir = (a - o).signum();
    let face = o + dir * axes(node.size)[f] / 2.0;
    let gap = (a - face) * dir;
    let d = lead + 2.0;
    (d + 1.0 < gap).then(|| Approach {
        f,
        depth: face + dir * d,
        side: dir > 0.0,
        from_cross: axes(toward)[c],
    })
}

/// Build an edge's control points: approach points (moved across by the
/// end's slot `off`), the routed waypoints, one straight lead-in per marked
/// end, then the clip onto each end shape.
fn route_edge(e: EdgeIn, ap: &[Option<Approach>; 2], off: [f64; 2]) -> EdgePath {
    let (start, end, pts) = &e.geom;
    if e.plain() {
        let label = clip_polyline(pts, |p| clip_end(p, start), |p| clip_end(p, end));
        let ctrl = match e.curve {
            EdgeCurve::Linear | EdgeCurve::Step => label.clone(),
            _ => basis_control_points(&label),
        };
        return EdgePath {
            ctrl,
            label,
            curve: e.curve,
            style: e.style,
        };
    }
    let c = e.axes().1;
    // Ends that line up across the gap join with one straight line; when
    // either had to move off its face centre, both move so it stays straight.
    let aligned = pts.len() == 2
        && ap.iter().all(Option::is_some)
        && (start.center_on(c) - end.center_on(c)).abs() < ALIGNED_EPS;
    // The shared shift stays on the narrower of the two faces.
    let room = (axes(start.size)[c].min(axes(end.size)[c]) / 2.0 - 4.0).max(0.0);
    let off = match off {
        [s, t] if aligned => {
            let o = if s.abs() >= t.abs() { s } else { t };
            [o.max(-room).min(room); 2]
        }
        off => off,
    };
    // A slotted rect end keeps its square entry, so it aims at its own slot;
    // round and rhombus ends always aim at the true centre.
    let aim = |which: usize| {
        let node = e.node(which);
        let mut center = node.center;
        if !matches!(node.shape, Some(s) if is_round(s)) {
            *axis_mut(&mut center, c) += off[which];
        }
        EndClip {
            center,
            size: node.size,
            shape: node.shape,
        }
    };
    let (aim_s, aim_t) = (aim(0), aim(1));
    // A round end aims from its slot on the face line, so the tip lands on
    // the slot with the head pointing at the centre; its approach point sits
    // on that radial ray just past the lead-in (held within the node's cross
    // extent), so the run under the marker stays straight.
    let round_tip = |which: usize| -> Option<(Pt, Pt)> {
        let (node, a) = (e.node(which), ap[which].as_ref()?);
        let shape = node.shape.filter(|&s| is_round(s))?;
        let mut q = node.center;
        *axis_mut(&mut q, a.f) +=
            (a.depth - node.center_on(a.f)).signum() * axes(node.size)[a.f] / 2.0;
        *axis_mut(&mut q, c) += round_cross(shape, off[which], node, a.f);
        let t = clip_end(q, node);
        let (dx, dy) = (t.0 - node.center.0, t.1 - node.center.1);
        Some((t, (dx / dx.hypot(dy), dy / dx.hypot(dy))))
    };
    let tips = [round_tip(0), round_tip(1)];
    let point = |which: usize| {
        ap[which].as_ref().map(|a| {
            let node = e.node(which);
            if let Some((t, u)) = tips[which] {
                let r = e.lead(which) + 2.0;
                let mut p = (t.0 + u.0 * r, t.1 + u.1 * r);
                let hc = axes(node.size)[c] / 2.0;
                let x = axes(p)[c] - node.center_on(c);
                *axis_mut(&mut p, c) = node.center_on(c) + x.max(-hc).min(hc);
                return p;
            }
            let mut p = node.center;
            *axis_mut(&mut p, a.f) = a.depth;
            *axis_mut(&mut p, c) += off[which];
            p
        })
    };
    let (a_s, a_t) = (point(0), point(1));
    let mut inner: Vec<Pt> = Vec::new();
    if !aligned {
        inner.extend(a_s);
        inner.extend(&pts[1..pts.len() - 1]);
        inner.extend(a_t);
    }
    // An end's tip, and the point its straight lead-in heads for: the next
    // control point, or straight out from the centre for a round end.
    let end = |which: usize, from: Pt| -> (Pt, Pt) {
        match tips[which] {
            Some((t, u)) => (t, (t.0 + u.0 * 1e3, t.1 + u.1 * 1e3)),
            None => (
                clip_end(from, if which == 0 { &aim_s } else { &aim_t }),
                from,
            ),
        }
    };
    let ctrl = match (inner.first(), inner.last()) {
        (Some(&first), Some(&last)) => {
            let ((p_s, out_s), (p_t, out_t)) = (end(0, first), end(1, last));
            let mut ctrl = vec![p_s];
            ctrl.extend(lead_in(p_s, out_s, e.lead(0)));
            ctrl.extend(&inner);
            ctrl.extend(lead_in(p_t, out_t, e.lead(1)));
            ctrl.push(p_t);
            ctrl
        }
        _ => vec![
            clip_end(aim_t.center, &aim_s),
            clip_end(aim_s.center, &aim_t),
        ],
    };
    EdgePath {
        label: ctrl.clone(),
        ctrl,
        curve: e.curve,
        style: e.style,
    }
}

impl EndClip {
    pub(super) fn center_on(&self, axis: usize) -> f64 {
        axes(self.center)[axis]
    }
}

pub(super) fn axes(p: Pt) -> [f64; 2] {
    [p.0, p.1]
}

fn axis_mut(p: &mut Pt, axis: usize) -> &mut f64 {
    if axis == 0 {
        &mut p.0
    } else {
        &mut p.1
    }
}

/// A point `len` out from the face point `face` toward `toward`, so the last
/// stretch under the marker is a straight run along the clip ray. `None` for
/// an unmarked end (`len` 0), or when `toward` is already that close: the run
/// to it is straight anyway.
fn lead_in(face: Pt, toward: Pt, len: f64) -> Option<Pt> {
    let (dx, dy) = (toward.0 - face.0, toward.1 - face.1);
    let d = dx.hypot(dy);
    (len > 0.0 && d > len + 1.0).then(|| (face.0 + dx / d * len, face.1 + dy / d * len))
}

/// How far a marker reaches back along the edge from its tip, per unit of
/// stroke width (markers scale with the stroke), from the defs in `edges.rs`.
fn marker_reach(head: EdgeHead) -> Option<f64> {
    match head {
        EdgeHead::Arrow => Some(12.0),
        EdgeHead::Circle => Some(10.0),
        EdgeHead::Cross => Some(5.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
