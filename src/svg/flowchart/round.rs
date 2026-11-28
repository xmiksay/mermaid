//! Round (circle family, rhombus) edge ends (#402). They take no face slot:
//! each is clipped on the ray from its neighbouring control point toward the
//! centre, as upstream does, so arrows spread around the shape by where they
//! come from. Afterwards, on every edge except self-loops (master-route ones
//! included), different-kind ends that would land on each other are turned
//! apart around the centre in 1° steps until clear.

use std::collections::BTreeMap;

use crate::parse::NodeShape;

use super::edges::{clip_end, EndClip};
use super::route::{EdgePath, Pt};

/// Gap kept between two different-kind heads, beyond their half-widths.
const HEAD_MARGIN: f64 = 4.0;
/// The furthest a tip is turned around its node to clear another head.
const MAX_TURN_DEG: u32 = 90;

pub(super) fn is_round(shape: NodeShape) -> bool {
    matches!(
        shape,
        NodeShape::Circle
            | NodeShape::DoubleCircle
            | NodeShape::FilledCircle
            | NodeShape::CrossedCircle
            | NodeShape::Rhombus
    )
}

/// One edge end on a round node.
pub(super) struct RoundEnd<'a> {
    pub(super) edge: usize,
    /// 0 for the start, 1 for the end.
    pub(super) which: usize,
    pub(super) id: &'a str,
    pub(super) node: EndClip,
    pub(super) kind: u32,
    pub(super) width: f64,
    /// The straight lead-in length under the marker (0 when unmarked).
    pub(super) lead: f64,
    /// A self-loop keeps its shape: its ends only block the others.
    pub(super) movable: bool,
}

/// Turn each round-node end that would land within both half-widths plus a
/// margin of an earlier, different-kind end on the same node around the
/// centre in 1° steps until clear (up to 90°, nearest angle first). Marked
/// heads are placed before bare line ends, so a line never leaves from a
/// head's tip. Its lead-in moves along onto the new ray. Same-kind tips may
/// coincide. Self-loops keep their shape. A turn whose new end span `clear`
/// rejects (edge, tip, the point past its lead-in) is passed over.
pub(super) fn nudge_round_tips(
    ends: &[RoundEnd],
    paths: &mut [Option<EdgePath>],
    clear: &dyn Fn(usize, &[Pt], usize) -> bool,
) {
    let mut by_node: BTreeMap<&str, Vec<&RoundEnd>> = BTreeMap::new();
    for end in ends {
        by_node.entry(end.id).or_default().push(end);
    }
    // Heads first: a bare line then turns away from a head, never a head
    // away from a line (stable, so edge order holds within each).
    for group in by_node.values_mut() {
        group.sort_by_key(|end| end.width == 0.0);
    }
    for group in by_node.values() {
        let Some(first) = group.first() else { continue };
        let mut placed = Placed::new(&first.node);
        for end in group {
            let Some(path) = paths[end.edge].as_mut() else {
                continue;
            };
            let n = path.ctrl.len();
            let at = if end.which == 0 { 0 } else { n - 1 };
            let tip = path.ctrl[at];
            // The curve a turned tip leaves along must keep off the nodes
            // the edge does not end on.
            let base = path.ctrl.clone();
            let keeps_clear = |t: Pt| {
                let mut c = base.clone();
                move_tip(&mut c, end, at, t);
                clear(end.edge, &c, end.which)
            };
            let new_tip = match placed.clash(end, tip) {
                Some((other, _)) if end.movable => {
                    turned_clear(end, tip, other, &|t| placed.clash(end, t), &keeps_clear)
                }
                _ => None,
            };
            let tip = match new_tip {
                Some(t) => {
                    move_tip(&mut path.ctrl, end, at, t);
                    path.label = path.ctrl.clone();
                    t
                }
                None => tip,
            };
            placed.add(tip, end.kind, end.width);
        }
    }
}

/// Angular buckets per head kind.
const BUCKETS: usize = 360;

/// The tips placed on one round node, bucketed by kind and by angle around
/// its centre, so a clash check only looks at different-kind tips nearby.
struct Placed {
    center: Pt,
    /// The smallest distance from the centre to the outline: any two tips
    /// a chord `d` apart are at most `2·asin(d / 2r)` apart in angle.
    inner: f64,
    /// `[kind][bucket]` → `(tip, width)`, and the widest marker per kind.
    tips: Vec<Vec<Vec<(Pt, f64)>>>,
    widest: Vec<f64>,
}

impl Placed {
    fn new(node: &EndClip) -> Self {
        let (hw, hh) = (node.size.0 / 2.0, node.size.1 / 2.0);
        let inner = if node.shape == Some(NodeShape::Rhombus) {
            hw * hh / hw.hypot(hh)
        } else {
            hw.min(hh)
        };
        Self {
            center: node.center,
            inner: inner.max(1e-6),
            tips: Vec::new(),
            widest: Vec::new(),
        }
    }

    fn bucket(&self, t: Pt) -> usize {
        let a = (t.1 - self.center.1)
            .atan2(t.0 - self.center.0)
            .to_degrees();
        (a.rem_euclid(360.0) as usize).min(BUCKETS - 1)
    }

    fn add(&mut self, t: Pt, kind: u32, width: f64) {
        let k = kind as usize;
        if self.tips.len() <= k {
            self.tips.resize_with(k + 1, || vec![Vec::new(); BUCKETS]);
            self.widest.resize(k + 1, 0.0);
        }
        let b = self.bucket(t);
        // A tip within 1px of a no-narrower same-kind one changes no clash
        // check by more than that pixel; skipping it keeps crowded nodes fast.
        let dup = self.tips[k][b]
            .iter()
            .any(|&(p, w)| w >= width && dist(p, t) < 1.0);
        if !dup {
            self.tips[k][b].push((t, width));
        }
        self.widest[k] = self.widest[k].max(width);
    }

    /// The first placed tip of another kind that `t` would land on, with
    /// its marker width.
    fn clash(&self, end: &RoundEnd, t: Pt) -> Option<(Pt, f64)> {
        let b = self.bucket(t) as i64;
        for (k, by_angle) in self.tips.iter().enumerate() {
            if k as u32 == end.kind {
                continue;
            }
            let clear = (self.widest[k] + end.width) / 2.0 + HEAD_MARGIN;
            let reach = 2.0 * (clear / (2.0 * self.inner)).min(1.0).asin().to_degrees();
            let span = (reach.ceil() as i64 + 1).min(BUCKETS as i64 / 2);
            for step in 0..=span {
                for b in [b + step, b - step] {
                    let tips = &by_angle[b.rem_euclid(BUCKETS as i64) as usize];
                    let hit = tips
                        .iter()
                        .find(|(p, w)| dist(*p, t) < (w + end.width) / 2.0 + HEAD_MARGIN);
                    if let Some(&hit) = hit {
                        return Some(hit);
                    }
                }
            }
        }
        None
    }
}

/// The nearest clear tip on the outline, turning in 1° steps on each side —
/// away from `other`, the head it landed on, first on a tie — up to 90°.
/// A step that lands on a head jumps straight past that head's blocked arc
/// (still on whole degrees), so a crowded node costs a few checks, not 180.
fn turned_clear(
    end: &RoundEnd,
    tip: Pt,
    other: Pt,
    clash: &dyn Fn(Pt) -> Option<(Pt, f64)>,
    clear: &dyn Fn(Pt) -> bool,
) -> Option<Pt> {
    let c = end.node.center;
    let angle = |p: Pt| (p.1 - c.1).atan2(p.0 - c.0);
    let a0 = angle(tip);
    let away = if (a0 - angle(other)).sin() >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let side = |s: f64| -> Option<(u32, Pt)> {
        let mut deg = 1;
        while deg <= MAX_TURN_DEG {
            let a = a0 + s * f64::from(deg).to_radians();
            let t = clip_end((c.0 + a.cos() * 1e4, c.1 + a.sin() * 1e4), &end.node);
            let Some((p, w)) = clash(t) else {
                if clear(t) {
                    return Some((deg, t));
                }
                deg += 1;
                continue;
            };
            // p blocks the outline within this angle of itself (exact on a
            // circle, near enough on a rhombus).
            let r = dist(t, c).min(dist(p, c)).max(1e-6);
            let clear = (w + end.width) / 2.0 + HEAD_MARGIN;
            let blocked = 2.0 * (clear / (2.0 * r)).min(1.0).asin();
            let ahead = (s * (angle(p) - a0)).to_degrees();
            let ahead = (ahead + 180.0).rem_euclid(360.0) - 180.0;
            let past = (ahead + blocked.to_degrees()).ceil().max(0.0) as u32;
            deg = past.clamp(deg + 1, MAX_TURN_DEG + 1);
        }
        None
    };
    match (side(away), side(-away)) {
        (Some((d1, t1)), Some((d2, t2))) => Some(if d1 <= d2 { t1 } else { t2 }),
        (one, two) => one.or(two).map(|(_, t)| t),
    }
}

/// Put the end's tip at `t`. A lead-in, if it has one, moves onto the line
/// from the new tip to the control point beyond it, so the run under the
/// head stays straight.
fn move_tip(ctrl: &mut Vec<Pt>, end: &RoundEnd, at: usize, t: Pt) {
    let n = ctrl.len();
    let (lead_at, beyond) = if end.which == 0 {
        (1, 2)
    } else {
        (n - 2, n.saturating_sub(3))
    };
    let old = ctrl[at];
    let has_lead = end.lead > 0.0 && n >= 3 && (dist(ctrl[lead_at], old) - end.lead).abs() < 1e-6;
    ctrl[at] = t;
    if has_lead {
        let toward = ctrl[beyond];
        let d = dist(toward, t);
        if d > end.lead + 1.0 {
            ctrl[lead_at] = (
                t.0 + (toward.0 - t.0) / d * end.lead,
                t.1 + (toward.1 - t.1) / d * end.lead,
            );
        } else {
            // As `lead_in`: too close for a lead-in; the run is straight.
            ctrl.remove(lead_at);
        }
    }
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::EdgeCurve;
    use crate::svg::style::resolve_edge_style;

    fn path(ctrl: Vec<Pt>) -> Option<EdgePath> {
        Some(EdgePath {
            label: ctrl.clone(),
            ctrl,
            curve: EdgeCurve::Basis,
            style: resolve_edge_style(&Vec::new(), None),
        })
    }

    fn end(edge: usize, kind: u32, lead: f64) -> RoundEnd<'static> {
        RoundEnd {
            edge,
            which: 1,
            id: "c",
            node: EndClip {
                center: (0.0, 0.0),
                size: (60.0, 60.0),
                shape: Some(NodeShape::Circle),
            },
            kind,
            width: 18.0,
            lead,
            movable: true,
        }
    }

    #[test]
    fn different_kind_tip_turns_just_clear_and_takes_its_lead_in_along() {
        // Two tips on top of a radius-30 circle, 2px apart; the second has a
        // 22px lead-in on its radial ray.
        let tip = (2.0, -(896f64.sqrt()));
        let lead_on_ray = (tip.0 * 52.0 / 30.0, tip.1 * 52.0 / 30.0);
        let mut paths = vec![
            path(vec![(0.0, -100.0), (0.0, -30.0)]),
            path(vec![(50.0, -100.0), lead_on_ray, tip]),
        ];
        nudge_round_tips(
            &[end(0, 1, 0.0), end(1, 2, 22.0)],
            &mut paths,
            &|_, _, _| true,
        );
        let moved = paths[1].as_ref().expect("path").ctrl.clone();
        let tip = moved[2];
        assert!(
            (dist(tip, (0.0, 0.0)) - 30.0).abs() < 1e-6,
            "on the outline"
        );
        assert!(tip.0 > 2.0, "turned away from the first tip");
        assert!(dist(tip, (0.0, -30.0)) >= 22.0, "clear: 9 + 9 + 4");
        assert!(dist(tip, (0.0, -30.0)) < 24.0, "but only just");
        // The lead-in follows onto the line from the new tip to (50, -100).
        let lead = moved[1];
        assert!((dist(lead, tip) - 22.0).abs() < 1e-6);
        let off_line = (lead.0 - tip.0) * (-100.0 - tip.1) - (lead.1 - tip.1) * (50.0 - tip.0);
        assert!(off_line.abs() < 1e-6);
    }

    #[test]
    fn same_kind_tips_may_coincide() {
        let mut paths = vec![
            path(vec![(0.0, -100.0), (0.0, -30.0)]),
            path(vec![(50.0, -100.0), (0.0, -30.0)]),
        ];
        nudge_round_tips(&[end(0, 1, 0.0), end(1, 1, 0.0)], &mut paths, &|_, _, _| {
            true
        });
        assert_eq!(paths[1].as_ref().expect("path").ctrl[1], (0.0, -30.0));
    }
}
