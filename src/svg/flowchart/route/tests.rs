use super::super::round::round_cross;
use super::*;
use crate::parse::NodeShape;
use crate::parse::{Diagram, FlowchartDiagram};
use crate::svg::style::resolve_edge_style;

const NO_LEADS: (f64, f64) = (0.0, 0.0);

fn rect(center: Pt, size: Pt) -> EndClip {
    EndClip {
        center,
        size,
        shape: Some(NodeShape::Rect),
    }
}

fn flow(src: &str) -> FlowchartDiagram {
    match crate::parse::parse(src).expect("parses") {
        Diagram::Flowchart(d) => d,
        _ => panic!("expected flowchart"),
    }
}

/// A TD edge `a → b` between two 60×40 rects, with `stroke-width: sw`.
fn edge_in<'a>(edge: &'a FlowEdge, sw: &str, pts: Vec<Pt>) -> EdgeIn<'a> {
    let (s, t) = (pts[0], pts[pts.len() - 1]);
    EdgeIn {
        geom: (rect(s, (60.0, 40.0)), rect(t, (60.0, 40.0)), pts),
        edge,
        curve: EdgeCurve::Basis,
        style: resolve_edge_style(&vec![("stroke-width".into(), sw.into())], None),
        horizontal: false,
        paired: false,
    }
}

#[test]
fn wide_boxes_in_later_rank_bend_on_flow_axis() {
    // dx > dy, but the ranks differ: the S bends on the flow axis (y).
    let (s, t) = (
        rect((0.0, 0.0), (300.0, 40.0)),
        rect((205.0, 125.0), (200.0, 40.0)),
    );
    assert_eq!(pair_depth(&s, &t, 1, NO_LEADS), Some(62.5));
}

#[test]
fn same_rank_and_overlapping_faces_draw_straight() {
    let s = rect((0.0, 0.0), (60.0, 40.0));
    assert_eq!(
        pair_depth(&s, &rect((200.0, 1.0), (60.0, 40.0)), 1, NO_LEADS),
        None
    );
    let tall = rect((0.0, 0.0), (60.0, 200.0));
    assert_eq!(
        pair_depth(&tall, &rect((100.0, 90.0), (60.0, 40.0)), 1, NO_LEADS),
        None
    );
}

#[test]
fn back_edge_and_horizontal_flow_bend_mid_gap() {
    let (s, t) = (
        rect((100.0, 200.0), (60.0, 40.0)),
        rect((0.0, 0.0), (60.0, 40.0)),
    );
    assert_eq!(pair_depth(&s, &t, 1, NO_LEADS), Some(100.0));
    let (s, t) = (
        rect((0.0, 0.0), (60.0, 40.0)),
        rect((200.0, 100.0), (60.0, 40.0)),
    );
    assert_eq!(pair_depth(&s, &t, 0, NO_LEADS), Some(100.0));
}

#[test]
fn tight_gap_moves_the_bend_off_the_marked_end() {
    // A 43px gap: the mid-gap bend would leave the arrow 21.5px, under its
    // 22px lead-in, so the bend moves toward the unmarked source.
    let (s, t) = (
        rect((0.0, 0.0), (60.0, 40.0)),
        rect((50.0, 83.0), (60.0, 40.0)),
    );
    assert_eq!(pair_depth(&s, &t, 1, (0.0, 22.0)), Some(39.0));
    assert_eq!(pair_depth(&s, &t, 1, NO_LEADS), Some(41.5));
    assert_eq!(
        pair_depth(&s, &t, 1, (22.0, 22.0)),
        None,
        "too tight: straight"
    );
    assert_eq!(pair_depth(&s, &t, 1, (f64::NAN, 0.0)), None);
}

#[test]
fn routed_end_approaches_just_past_its_lead_in() {
    let node = rect((0.0, 0.0), (60.0, 40.0));
    // Dummy 100 below the bottom face (y 20), offset sideways: the approach
    // sits 22 + 2 below the face, on the node's own column.
    let a = end_approach(&node, (80.0, 120.0), 1, 0, 22.0).expect("room");
    assert_eq!((a.depth, a.side, a.from_cross), (44.0, true, 80.0));
    // A waypoint inside the lead-in reach gets no approach point.
    assert!(end_approach(&node, (80.0, 40.0), 1, 0, 22.0).is_none());
}

#[test]
fn routed_end_enters_square_on() {
    let d = flow("flowchart TD\na --> b");
    let e = edge_in(
        &d.edges[0],
        "1.5",
        vec![(0.0, 0.0), (90.0, 100.0), (0.0, 200.0)],
    );
    let ap = approaches(&e);
    let path = route_edge(e, &ap, [0.0; 2]);
    let n = path.ctrl.len();
    // Tip on the top face of `b`, straight down from the lead-in.
    assert_eq!(path.ctrl[n - 1], (0.0, 180.0));
    assert_eq!(path.ctrl[n - 2], (0.0, 158.0));
}

#[test]
fn slot_offset_moves_rect_end_but_round_end_aims_at_centre() {
    let d = flow("flowchart TD\na --> b");
    let e = edge_in(&d.edges[0], "1.5", vec![(0.0, 0.0), (0.0, 200.0)]);
    let ap = approaches(&e);
    let path = route_edge(e, &ap, [0.0, 11.0]);
    assert_eq!(
        path.ctrl[path.ctrl.len() - 1],
        (11.0, 180.0),
        "square-on in its slot"
    );

    let mut e = edge_in(&d.edges[0], "1.5", vec![(0.0, 0.0), (0.0, 200.0)]);
    e.geom.1.shape = Some(NodeShape::Circle);
    let ap = approaches(&e);
    let path = route_edge(e, &ap, [0.0, 11.0]);
    let (tip, lead) = (
        path.ctrl[path.ctrl.len() - 1],
        path.ctrl[path.ctrl.len() - 2],
    );
    // Tip and lead-in on one ray through the centre (0, 200).
    let cross = (lead.0 - tip.0) * (200.0 - tip.1) - (lead.1 - tip.1) * (0.0 - tip.0);
    assert!(cross.abs() < 1e-9, "round end aims at the centre");
}

#[test]
fn lead_in_sits_on_the_clip_ray() {
    assert_eq!(lead_in((0.0, 0.0), (0.0, -40.0), 22.0), Some((0.0, -22.0)));
    assert_eq!(lead_in((0.0, 0.0), (0.0, -20.0), 22.0), None);
    assert_eq!(lead_in((0.0, 0.0), (0.0, -40.0), 0.0), None, "unmarked end");
}

#[test]
fn stroke_width_accepts_only_finite_non_negative_overrides() {
    let d = flow("flowchart TD\na ==> b");
    let pts = || vec![(0.0, 0.0), (0.0, 200.0)];
    assert_eq!(edge_in(&d.edges[0], "4px", pts()).stroke(), 4.0);
    assert_eq!(edge_in(&d.edges[0], "0", pts()).stroke(), 0.0);
    for bad in ["NaN", "inf", "-2", "thin"] {
        assert_eq!(
            edge_in(&d.edges[0], bad, pts()).stroke(),
            3.0,
            "{bad}: thick default"
        );
    }
}

#[test]
fn round_cross_lands_the_tip_on_the_slot_and_stays_bounded() {
    let circle = EndClip {
        center: (0.0, 0.0),
        size: (60.0, 60.0),
        shape: Some(NodeShape::Circle),
    };
    // Tip 6 across on a radius-30 circle, aimed from the face line 30 out:
    // x = 6·30/√(900 − 36).
    let x = round_cross(NodeShape::Circle, 6.0, &circle, 1);
    assert!((x - 6.0 * 30.0 / 864f64.sqrt()).abs() < 1e-9);
    // A far slot is held to 0.6 of the radius: x = 18·30/24.
    assert_eq!(round_cross(NodeShape::Circle, 50.0, &circle, 1), 22.5);
    assert_eq!(round_cross(NodeShape::Circle, -50.0, &circle, 1), -22.5);
    let rhombus = EndClip {
        center: (0.0, 0.0),
        size: (40.0, 30.0),
        shape: Some(NodeShape::Rhombus),
    };
    // Small rhombus: the 1/(1 − |off|/hc) blow-up stops at hc = 20.
    assert_eq!(round_cross(NodeShape::Rhombus, 19.0, &rhombus, 1), 20.0);
    assert_eq!(round_cross(NodeShape::Rhombus, 0.0, &rhombus, 1), 0.0);
}

#[test]
fn skips_ranks_only_for_bare_lines_across_more_than_two_layer_gaps() {
    let d = flow("flowchart TD\na --> b");
    let near = edge_in(&d.edges[0], "1.5", vec![(0.0, 0.0), (50.0, 120.0)]);
    assert!(!near.skips_ranks(), "one layer gap");
    let far = edge_in(&d.edges[0], "1.5", vec![(0.0, 0.0), (50.0, 300.0)]);
    assert!(far.skips_ranks(), "a 260px gap skips ranks");
    let routed = edge_in(
        &d.edges[0],
        "1.5",
        vec![(0.0, 0.0), (20.0, 150.0), (50.0, 300.0)],
    );
    assert!(!routed.skips_ranks(), "the layout routed it");
}
