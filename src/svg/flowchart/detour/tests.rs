use super::*;
use crate::svg::style::resolve_edge_style;

fn clip(center: Pt, size: (f64, f64), shape: NodeShape) -> EndClip {
    EndClip {
        center,
        size,
        shape: Some(shape),
    }
}

fn rect(center: Pt) -> EndClip {
    clip(center, (60.0, 40.0), NodeShape::Rect)
}

/// An edge `a → c` with an arrowhead (lead-in 22) at `c`.
fn edge(a: EndClip, c: EndClip) -> DetourEdge<'static> {
    DetourEdge {
        ends: ["a", "c"],
        clips: [a, c],
        leads: [0.0, 22.0],
        flow: 1,
    }
}

fn path(ctrl: Vec<Pt>, curve: EdgeCurve) -> Option<EdgePath> {
    Some(EdgePath {
        label: ctrl.clone(),
        ctrl,
        curve,
        style: resolve_edge_style(&Vec::new(), None),
    })
}

/// Detours one edge among `nodes` (`a`, `c` and the obstacles `b…`).
fn run(ctrl: Vec<Pt>, curve: EdgeCurve, e: DetourEdge, nodes: &[(&str, EndClip)]) -> EdgePath {
    let obstacles: Vec<Obstacle> = nodes
        .iter()
        .map(|&(id, node)| Obstacle { id, node })
        .collect();
    let mut paths = vec![path(ctrl, curve)];
    detour_all(&mut paths, &[Some(e)], &obstacles, &[]);
    paths.pop().flatten().expect("path")
}

fn hit_count(p: &EdgePath, nodes: &[(&str, EndClip)]) -> usize {
    let bodies = nodes
        .iter()
        .filter_map(|&(id, n)| Some((id, Body::new(&n)?)))
        .collect();
    Scene::new(bodies, Vec::new())
        .hits(&drawn(&p.ctrl, p.curve), true, 0.0, |_, _| false)
        .len()
}

/// A column A (y 0) → C (y 300) with B in between, 10px right of the line.
fn column() -> (Vec<Pt>, [(&'static str, EndClip); 3]) {
    let (a, b, c) = (rect((0.0, 0.0)), rect((10.0, 150.0)), rect((0.0, 300.0)));
    (
        vec![(0.0, 20.0), (0.0, 280.0)],
        [("a", a), ("b", b), ("c", c)],
    )
}

#[test]
fn edge_through_a_node_goes_round_it_on_the_shorter_side() {
    let (ctrl, nodes) = column();
    for curve in [EdgeCurve::Basis, EdgeCurve::Linear, EdgeCurve::Step] {
        let before = path(ctrl.clone(), curve).expect("path");
        assert_eq!(hit_count(&before, &nodes[1..2]), 1, "{curve:?}");
        let p = run(ctrl.clone(), curve, edge(nodes[0].1, nodes[2].1), &nodes);
        assert_eq!(hit_count(&p, &nodes[1..2]), 0, "{curve:?} clears B");
        // B sits right of the line, so the detour passes on its left.
        assert!(p.ctrl.iter().all(|q| q.0 <= 0.0), "{curve:?}: {:?}", p.ctrl);
        // The marked end keeps its tip; the bare one may leave A's outline
        // elsewhere, heading for the detour.
        assert_eq!(p.ctrl.last(), ctrl.last());
        let start = p.ctrl.first().copied().expect("start");
        let on_a = (start.0.abs() - 30.0).abs() < 1e-6 || (start.1.abs() - 20.0).abs() < 1e-6;
        assert!(
            on_a && start.0.abs() <= 30.0 && start.1.abs() <= 20.0,
            "{start:?}"
        );
        assert_eq!(p.label, p.ctrl, "the label follows the detour");
    }
}

#[test]
fn the_end_run_under_a_marker_stays_straight() {
    let (ctrl, nodes) = column();
    let p = run(ctrl, EdgeCurve::Basis, edge(nodes[0].1, nodes[2].1), &nodes);
    let n = p.ctrl.len();
    // Tip, one and two lead-ins out, all on the original vertical line.
    let tail = &p.ctrl[n - 3..];
    assert_eq!(tail, &[(0.0, 236.0), (0.0, 258.0), (0.0, 280.0)]);
}

#[test]
fn blocked_side_takes_the_other_one() {
    let (ctrl, nodes) = column();
    // A wall just left of B blocks the shorter side.
    let wall = clip((-56.0, 150.0), (70.0, 120.0), NodeShape::Rect);
    let nodes = [nodes[0], nodes[1], ("w", wall), nodes[2]];
    let p = run(ctrl, EdgeCurve::Basis, edge(nodes[0].1, nodes[3].1), &nodes);
    assert_eq!(hit_count(&p, &nodes[1..3]), 0);
    assert!(
        p.ctrl.iter().any(|q| q.0 > 40.0),
        "went right: {:?}",
        p.ctrl
    );
}

#[test]
fn clear_and_degenerate_edges_stay_as_they_are() {
    let (_, nodes) = column();
    let e = || edge(nodes[0].1, nodes[2].1);
    let clear = vec![(-200.0, 20.0), (-200.0, 280.0)];
    assert_eq!(
        run(clear.clone(), EdgeCurve::Basis, e(), &nodes).ctrl,
        clear
    );
    let nan = vec![(f64::NAN, 20.0), (0.0, 280.0)];
    assert_eq!(run(nan, EdgeCurve::Basis, e(), &nodes).ctrl.len(), 2);
    let huge = vec![(0.0, -1e300), (0.0, 1e300)];
    assert_eq!(run(huge, EdgeCurve::Basis, e(), &nodes).ctrl.len(), 2);
}

#[test]
fn a_round_end_is_aimed_again_from_its_new_neighbour() {
    let (ctrl, nodes) = column();
    let c = clip((0.0, 300.0), (40.0, 40.0), NodeShape::Circle);
    let nodes = [nodes[0], nodes[1], ("c", c)];
    let p = run(ctrl, EdgeCurve::Basis, edge(nodes[0].1, c), &nodes);
    let n = p.ctrl.len();
    let (tip, lead, next) = (p.ctrl[n - 1], p.ctrl[n - 2], p.ctrl[n - 4]);
    assert!((dist(tip, c.center) - 20.0).abs() < 1e-6, "on the circle");
    assert!((dist(tip, lead) - 22.0).abs() < 1e-6);
    // Tip, lead-in and the point the curve comes from lie on one ray.
    let cross = (lead.0 - tip.0) * (next.1 - tip.1) - (lead.1 - tip.1) * (next.0 - tip.0);
    assert!(cross.abs() < 1e-6, "{:?}", p.ctrl);
}

#[test]
fn a_subgraph_end_owns_its_members_only_for_a_member() {
    let (ctrl, nodes) = column();
    let frame = |center: Pt| EndClip {
        center,
        size: (200.0, 200.0),
        shape: None,
    };
    let to = |g: EndClip| DetourEdge {
        ends: ["a", "g"],
        clips: [nodes[0].1, g],
        leads: [0.0, 22.0],
        flow: 1,
    };
    // A sits inside the frame too: B is part of where the edge goes.
    let inner = to(frame((0.0, 100.0)));
    assert_eq!(
        run(ctrl.clone(), EdgeCurve::Basis, inner, &nodes).ctrl,
        ctrl
    );
    // From outside, the edge still keeps off the frame's members.
    let outer = to(frame((0.0, 200.0)));
    let p = run(ctrl, EdgeCurve::Basis, outer, &nodes);
    assert_eq!(hit_count(&p, &nodes[1..2]), 0);
}

#[test]
fn other_markers_are_kept_clear_of_but_never_trigger_a_detour() {
    let (ctrl, nodes) = column();
    let e = || edge(nodes[0].1, nodes[2].1);
    // Another edge's arrowhead right on the line: no detour for that alone.
    let mut paths = vec![
        path(ctrl.clone(), EdgeCurve::Basis),
        path(vec![(-100.0, 100.0), (0.0, 100.0)], EdgeCurve::Basis),
    ];
    let other = DetourEdge {
        ends: ["x", "y"],
        clips: [nodes[0].1, nodes[2].1],
        leads: [0.0, 22.0],
        flow: 1,
    };
    let far: Vec<Obstacle> = [("a", nodes[0].1), ("c", nodes[2].1)]
        .iter()
        .map(|&(id, node)| Obstacle { id, node })
        .collect();
    detour_all(&mut paths, &[Some(e()), Some(other)], &far, &[]);
    assert_eq!(paths[0].as_ref().expect("path").ctrl, ctrl);
}

#[test]
fn a_detour_keeps_clear_of_other_markers() {
    let (ctrl, nodes) = column();
    // Another edge's arrowhead points at B from the left, on the short side.
    let mut paths = vec![
        path(ctrl, EdgeCurve::Basis),
        path(vec![(-125.0, 150.0), (-25.0, 150.0)], EdgeCurve::Basis),
    ];
    let other = DetourEdge {
        ends: ["x", "b"],
        clips: [nodes[0].1, nodes[1].1],
        leads: [0.0, 22.0],
        flow: 1,
    };
    let obstacles: Vec<Obstacle> = nodes
        .iter()
        .map(|&(id, node)| Obstacle { id, node })
        .collect();
    detour_all(
        &mut paths,
        &[Some(edge(nodes[0].1, nodes[2].1)), Some(other)],
        &obstacles,
        &[],
    );
    let p = paths[0].as_ref().expect("path");
    let head = clip((-34.0, 150.0), (26.0, 26.0), NodeShape::Circle);
    assert_eq!(hit_count(p, &nodes[1..2]), 0);
    assert_eq!(hit_count(p, &[("h", head)]), 0, "{:?}", p.ctrl);
}

#[test]
fn a_far_end_behind_a_wide_node_is_reached_round_its_end() {
    // A wide node right between A and D, D under its right part: no move
    // across the crossing clears it, only going round its right end.
    let a = rect((0.0, 0.0));
    let wide = clip((300.0, 150.0), (700.0, 44.0), NodeShape::Rect);
    let d = rect((500.0, 300.0));
    let nodes = [("a", a), ("b", wide), ("c", d)];
    let ctrl = vec![(30.0, 10.0), (500.0, 236.0), (500.0, 258.0), (500.0, 280.0)];
    let p = run(ctrl, EdgeCurve::Basis, edge(a, d), &nodes);
    assert_eq!(hit_count(&p, &nodes[1..2]), 0, "{:?}", p.ctrl);
    assert!(
        p.ctrl.iter().any(|q| q.0 > 650.0),
        "round the end: {:?}",
        p.ctrl
    );
    let (_, back) = measure(&drawn(&p.ctrl, p.curve), 1, ((30.0, 10.0), (500.0, 280.0)));
    assert!(back <= BACKTRACK, "never back up the flow: {back}");
}

#[test]
fn a_step_detour_puts_its_leg_on_the_clearance_line() {
    let (ctrl, nodes) = column();
    let p = run(ctrl, EdgeCurve::Step, edge(nodes[0].1, nodes[2].1), &nodes);
    // B spans x -20..40: the vertical leg runs half a gap clear of it.
    let gap = LayoutConfig::default().node_gap;
    let legs: Vec<f64> = p
        .ctrl
        .windows(2)
        .filter(|w| w[0].0 == w[1].0)
        .map(|w| w[0].0)
        .collect();
    assert!(legs.contains(&(-20.0 - gap / 2.0)), "{:?}", p.ctrl);
}

#[test]
fn backtrack_counts_only_travel_against_the_flow() {
    let tips = ((0.0, 0.0), (0.0, 100.0));
    // Out sideways and back is fine; up against a top-down flow is not.
    let sideways = [(0.0, 0.0), (80.0, 50.0), (0.0, 100.0)];
    assert_eq!(measure(&sideways, 1, tips).1, 0.0);
    let hook = [(0.0, 0.0), (0.0, -30.0), (0.0, 100.0)];
    assert_eq!(measure(&hook, 1, tips).1, 30.0);
}
