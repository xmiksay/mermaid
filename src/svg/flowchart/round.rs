//! Round (circle, rhombus) edge ends: where their approach point goes so the
//! tip lands on its slot, and when they keep the plain ray instead (#402).

use crate::parse::NodeShape;
use crate::svg::label_layout::segment_rect_overlap;

use super::edges::EndClip;
use super::route::{axes, Approach, EdgeIn, Pt};
use super::slots::ROUND_REACH;

/// The cross offset, on the face line, of the point a round end aims from so
/// that its ray to the centre lands the tip at slot offset `off` (solved for
/// the circle or rhombus outline). The slot stays within `ROUND_REACH` of
/// the half-extent and the point never leaves the node's own cross extent,
/// so a small node cannot swing its edge wide.
pub(super) fn round_cross(shape: NodeShape, off: f64, node: &EndClip, f: usize) -> f64 {
    let (hf, hc) = (axes(node.size)[f] / 2.0, axes(node.size)[1 - f] / 2.0);
    let half = if shape == NodeShape::Rhombus { hc } else { hf };
    let off = off.max(-ROUND_REACH * half).min(ROUND_REACH * half);
    let x = if shape == NodeShape::Rhombus {
        off / (1.0 - off.abs() / hc)
    } else {
        off * hf / (hf * hf - off * off).sqrt()
    };
    x.max(-hc).min(hc)
}

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

/// A round end's approach, dropped where the run from its neighbouring
/// control point into it would cross another node: the plain ray from that
/// neighbour then aims the end at its centre instead. A single-end choice;
/// rect ends always keep their square entry.
pub(super) fn clear_of_nodes(
    e: &EdgeIn,
    mut ap: [Option<Approach>; 2],
    nodes: &[(Pt, Pt)],
) -> [Option<Approach>; 2] {
    let pts = &e.geom.2;
    let point = |which: usize, a: &Approach| {
        let mut p = axes(e.node(which).center);
        p[a.f] = a.depth;
        (p[0], p[1])
    };
    let at: [Option<Pt>; 2] = [0, 1].map(|w| ap[w].as_ref().map(|a| point(w, a)));
    for which in 0..2 {
        let (Some(a), Some(shape)) = (at[which], e.node(which).shape) else {
            continue;
        };
        if !is_round(shape) {
            continue;
        }
        let neighbour = match (pts.len(), at[1 - which]) {
            (2, Some(other)) => other,
            (2, None) => e.node(1 - which).center,
            (n, _) => pts[if which == 0 { 1 } else { n - 2 }],
        };
        let own = [e.node(0).center, e.node(1).center];
        let blocked = nodes.iter().any(|&(c, (w, h))| {
            !own.contains(&c)
                && segment_rect_overlap(neighbour, a, (c.0 - w / 2.0, c.1 - h / 2.0, w, h)) > 0.0
        });
        if blocked {
            ap[which] = None;
        }
    }
    ap
}
