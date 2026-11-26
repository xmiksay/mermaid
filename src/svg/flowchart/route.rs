//! Flowchart edge geometry: the control points an edge is drawn through, built
//! in centre space and clipped to the end shapes last. Shared by the
//! canvas-measuring pass and the drawing pass.

use crate::parse::{EdgeCurve, EdgeHead, EdgeLine, FlowEdge};
use crate::svg::style::ResolvedStyle;

use super::edges::{clip_end, EdgeGeometry, EndClip};

type Pt = (f64, f64);

/// Below this cross-axis offset two ends are effectively aligned and the
/// edge draws straight.
const ALIGNED_EPS: f64 = 2.0;
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
}

/// Resolve an edge's drawn control points. `paired` marks a self-loop or one
/// half of an opposite pair: those, cluster ends and linear/step curves keep
/// the plain clip-first route.
pub(super) fn route_edge(
    (start, end, pts): EdgeGeometry,
    edge: &FlowEdge,
    curve: EdgeCurve,
    style: ResolvedStyle,
    horizontal: bool,
    paired: bool,
) -> EdgePath {
    let plain = paired
        || start.shape.is_none()
        || end.shape.is_none()
        || matches!(curve, EdgeCurve::Linear | EdgeCurve::Step);
    if plain {
        let label = clip_edge_polyline(&pts, &start, &end);
        let ctrl = match curve {
            EdgeCurve::Linear | EdgeCurve::Step => label.clone(),
            _ => basis_control_points(&label),
        };
        return EdgePath {
            ctrl,
            label,
            curve,
            style,
        };
    }

    let width = stroke_width(edge, &style);
    let lead = |head: EdgeHead| marker_reach(head).map_or(0.0, |r| r * width + LEAD_MARGIN);
    let leads = (lead(edge.tail), lead(edge.head));
    let inner: Vec<Pt> = if pts.len() == 2 {
        s_points(&start, &end, horizontal, leads)
    } else {
        pts[1..pts.len() - 1].to_vec()
    };
    let ctrl = match (inner.first(), inner.last()) {
        (Some(&first), Some(&last)) => {
            let p_s = clip_end(first, &start);
            let p_t = clip_end(last, &end);
            let mut ctrl = vec![p_s];
            ctrl.extend(lead_in(p_s, first, leads.0));
            ctrl.extend(&inner);
            ctrl.extend(lead_in(p_t, last, leads.1));
            ctrl.push(p_t);
            ctrl
        }
        _ => vec![clip_end(end.center, &start), clip_end(start.center, &end)],
    };
    EdgePath {
        label: ctrl.clone(),
        ctrl,
        curve,
        style,
    }
}

/// The two S-bend points `[A_s, A_t]` of a two-point edge. The bend axis
/// comes from the rank relation, not the geometry: ends in different ranks
/// (a forward or a back edge) bend across the flow axis, so each end meets
/// its flow face square-on however wide the boxes are. Ends in one rank share
/// the flow coordinate and join straight between their side faces.
///
/// The bend sits mid-way through the gap between the two facing faces, moved
/// toward an unmarked end when that is what leaves each marked end room for
/// its straight lead-in (`leads`, start and end). A gap too tight for both
/// draws straight, aimed at the centres, which is straight under any marker.
fn s_points(start: &EndClip, end: &EndClip, horizontal: bool, leads: (f64, f64)) -> Vec<Pt> {
    let (f, c) = if horizontal { (0, 1) } else { (1, 0) };
    let (s, t) = (axes(start.center), axes(end.center));
    if (t[f] - s[f]).abs() <= ALIGNED_EPS || (t[c] - s[c]).abs() < ALIGNED_EPS {
        return Vec::new();
    }
    let (sh, th) = (axes(start.size)[f] / 2.0, axes(end.size)[f] / 2.0);
    let dir = (t[f] - s[f]).signum();
    let lo = s[f] + dir * sh;
    let gap = (t[f] - dir * th - lo) * dir;
    let (min_s, min_t) = (leads.0 + 2.0, leads.1 + 2.0);
    if gap < min_s + min_t {
        return Vec::new();
    }
    let mid = lo + dir * (gap / 2.0).clamp(min_s, gap - min_t);
    let at = |cross: f64| if f == 0 { (mid, cross) } else { (cross, mid) };
    vec![at(s[c]), at(t[c])]
}

fn axes(p: Pt) -> [f64; 2] {
    [p.0, p.1]
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

/// The edge's stroke width: a `linkStyle` override, else the line kind's.
fn stroke_width(edge: &FlowEdge, style: &ResolvedStyle) -> f64 {
    let default = if edge.line == EdgeLine::Thick {
        3.0
    } else {
        1.5
    };
    style
        .stroke_width
        .as_deref()
        .and_then(|w| w.trim().parse().ok())
        .unwrap_or(default)
}

/// Clips a waypoint polyline's first/last segment to each endpoint's shape.
fn clip_edge_polyline(pts: &[Pt], start: &EndClip, end: &EndClip) -> Vec<Pt> {
    let n = pts.len();
    let mut clipped = Vec::with_capacity(n);
    clipped.push(clip_end(pts[1], start));
    clipped.extend(&pts[1..n - 1]);
    clipped.push(clip_end(pts[n - 2], end));
    clipped
}

/// Upstream renders every edge through `curveBasis`, so a bare two-point edge
/// between offset nodes leaves and re-enters along its dominant axis as a
/// gentle S rather than a straight diagonal. Only the plain (clip-first)
/// route uses this; aligned ends stay collinear and draw straight.
fn basis_control_points(pts: &[Pt]) -> Vec<Pt> {
    if pts.len() != 2 {
        return pts.to_vec();
    }
    let ((ax, ay), (bx, by)) = (pts[0], pts[1]);
    let (dx, dy) = (bx - ax, by - ay);
    if dy.abs() >= dx.abs() {
        if dx.abs() < ALIGNED_EPS {
            return pts.to_vec();
        }
        let my = (ay + by) / 2.0;
        vec![(ax, ay), (ax, my), (bx, my), (bx, by)]
    } else {
        if dy.abs() < ALIGNED_EPS {
            return pts.to_vec();
        }
        let mx = (ax + bx) / 2.0;
        vec![(ax, ay), (mx, ay), (mx, by), (bx, by)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::NodeShape;

    const NO_LEADS: (f64, f64) = (0.0, 0.0);

    fn rect(center: Pt, size: Pt) -> EndClip {
        EndClip {
            center,
            size,
            shape: Some(NodeShape::Rect),
        }
    }

    #[test]
    fn wide_boxes_in_later_rank_bend_on_flow_axis() {
        // dx > dy, but the ranks differ: the S must bend vertically.
        let (s, t) = (
            rect((0.0, 0.0), (300.0, 40.0)),
            rect((205.0, 125.0), (200.0, 40.0)),
        );
        assert_eq!(
            s_points(&s, &t, false, NO_LEADS),
            vec![(0.0, 62.5), (205.0, 62.5)]
        );
    }

    #[test]
    fn same_rank_and_aligned_ends_draw_straight() {
        let s = rect((0.0, 0.0), (60.0, 40.0));
        assert!(s_points(&s, &rect((200.0, 1.0), (60.0, 40.0)), false, NO_LEADS).is_empty());
        assert!(s_points(&s, &rect((1.0, 200.0), (60.0, 40.0)), false, NO_LEADS).is_empty());
    }

    #[test]
    fn horizontal_flow_bends_on_x() {
        let (s, t) = (
            rect((0.0, 0.0), (60.0, 40.0)),
            rect((200.0, 100.0), (60.0, 40.0)),
        );
        assert_eq!(
            s_points(&s, &t, true, NO_LEADS),
            vec![(100.0, 0.0), (100.0, 100.0)]
        );
    }

    #[test]
    fn back_edge_bends_between_opposite_faces() {
        let (s, t) = (
            rect((100.0, 200.0), (60.0, 40.0)),
            rect((0.0, 0.0), (60.0, 40.0)),
        );
        assert_eq!(
            s_points(&s, &t, false, NO_LEADS),
            vec![(100.0, 100.0), (0.0, 100.0)]
        );
    }

    #[test]
    fn overlapping_faces_draw_straight() {
        let (s, t) = (
            rect((0.0, 0.0), (60.0, 200.0)),
            rect((100.0, 90.0), (60.0, 40.0)),
        );
        assert!(s_points(&s, &t, false, NO_LEADS).is_empty());
    }

    #[test]
    fn tight_gap_moves_the_bend_off_the_marked_end() {
        // A 43px gap: the mid-gap bend would leave the arrow 21.5px, under
        // its 22px lead-in, so the bend moves toward the unmarked source.
        let (s, t) = (
            rect((0.0, 0.0), (60.0, 40.0)),
            rect((50.0, 83.0), (60.0, 40.0)),
        );
        assert_eq!(
            s_points(&s, &t, false, (0.0, 22.0)),
            vec![(0.0, 39.0), (50.0, 39.0)]
        );
        assert_eq!(
            s_points(&s, &t, false, NO_LEADS),
            vec![(0.0, 41.5), (50.0, 41.5)]
        );
        // Too tight for both leads: straight.
        assert!(s_points(&s, &t, false, (22.0, 22.0)).is_empty());
    }

    #[test]
    fn lead_in_sits_on_the_clip_ray() {
        assert_eq!(lead_in((0.0, 0.0), (0.0, -40.0), 22.0), Some((0.0, -22.0)));
        assert_eq!(lead_in((0.0, 0.0), (0.0, -20.0), 22.0), None);
        assert_eq!(lead_in((0.0, 0.0), (0.0, -40.0), 0.0), None, "unmarked end");
    }

    #[test]
    fn stroke_width_reads_link_style_override() {
        let crate::parse::Diagram::Flowchart(d) =
            crate::parse::parse("flowchart TD\na ==> b").expect("parses")
        else {
            panic!("expected flowchart");
        };
        let edge = &d.edges[0];
        let none = crate::svg::style::resolve_edge_style(&Vec::new(), None);
        assert_eq!(stroke_width(edge, &none), 3.0, "thick default");
        let style = crate::svg::style::resolve_edge_style(
            &vec![("stroke-width".into(), "4px".into())],
            None,
        );
        assert_eq!(stroke_width(edge, &style), 4.0);
    }
}
