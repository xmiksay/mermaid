//! Round (circle family, rhombus) edge ends (#402). They take no face slot:
//! each is clipped on the ray from its neighbouring control point toward the
//! centre, as upstream does, so arrows spread around the shape by where they
//! come from. Only different-kind tips that would land on each other are
//! nudged apart, by a small turn around the centre.

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
/// margin of an earlier, different-kind end on the same node just far
/// enough around the centre to clear it. Marked heads are placed before bare
/// line ends, so a line never leaves from a head's tip. Its lead-in
/// moves along onto the new ray. Same-kind tips may coincide.
pub(super) fn nudge_round_tips(ends: &[RoundEnd], paths: &mut [Option<EdgePath>]) {
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
        let mut placed: Vec<(Pt, u32, f64)> = Vec::new();
        for end in group {
            let Some(path) = paths[end.edge].as_mut() else {
                continue;
            };
            let n = path.ctrl.len();
            let at = if end.which == 0 { 0 } else { n - 1 };
            let tip = path.ctrl[at];
            let clash = |t: Pt| {
                placed.iter().find(|(p, k, w)| {
                    *k != end.kind && dist(*p, t) < (w + end.width) / 2.0 + HEAD_MARGIN
                })
            };
            let new_tip = match clash(tip) {
                Some(&(other, ..)) if end.movable => {
                    turned_clear(end, tip, other, &|t| clash(t).is_some())
                }
                _ => None,
            };
            let tip = match new_tip {
                Some(t) => {
                    move_tip(path, end, at, t);
                    t
                }
                None => tip,
            };
            placed.push((tip, end.kind, end.width));
        }
    }
}

/// The nearest tip on the outline, turning degree by degree — first away
/// from `other`, the head it landed on, then the other way — that no longer
/// `clashes` with a placed head.
fn turned_clear(end: &RoundEnd, tip: Pt, other: Pt, clashes: &dyn Fn(Pt) -> bool) -> Option<Pt> {
    let c = end.node.center;
    let (a0, ao) = (
        (tip.1 - c.1).atan2(tip.0 - c.0),
        (other.1 - c.1).atan2(other.0 - c.0),
    );
    let away = if (a0 - ao).sin() >= 0.0 { 1.0 } else { -1.0 };
    (1..=MAX_TURN_DEG).find_map(|deg| {
        [away, -away].into_iter().find_map(|side| {
            let a = a0 + side * f64::from(deg).to_radians();
            let t = clip_end((c.0 + a.cos() * 1e4, c.1 + a.sin() * 1e4), &end.node);
            (!clashes(t)).then_some(t)
        })
    })
}

/// Put the end's tip at `t`. A lead-in, if it has one, moves onto the line
/// from the new tip to the control point beyond it, so the run under the
/// head stays straight.
fn move_tip(path: &mut EdgePath, end: &RoundEnd, at: usize, t: Pt) {
    let n = path.ctrl.len();
    let (lead_at, beyond) = if end.which == 0 {
        (1, 2)
    } else {
        (n - 2, n.saturating_sub(3))
    };
    let old = path.ctrl[at];
    let has_lead =
        end.lead > 0.0 && n >= 3 && (dist(path.ctrl[lead_at], old) - end.lead).abs() < 1e-6;
    path.ctrl[at] = t;
    if has_lead {
        let toward = path.ctrl[beyond];
        let d = dist(toward, t).max(1e-9);
        path.ctrl[lead_at] = (
            t.0 + (toward.0 - t.0) / d * end.lead,
            t.1 + (toward.1 - t.1) / d * end.lead,
        );
    }
    path.label = path.ctrl.clone();
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
        nudge_round_tips(&[end(0, 1, 0.0), end(1, 2, 22.0)], &mut paths);
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
        nudge_round_tips(&[end(0, 1, 0.0), end(1, 1, 0.0)], &mut paths);
        assert_eq!(paths[1].as_ref().expect("path").ctrl[1], (0.0, -30.0));
    }
}
