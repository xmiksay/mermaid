//! Review repros for #396 (`e1`–`e6`), each checked in both layout modes
//! against the shared scene invariants.

use std::sync::mpsc;
use std::time::Duration;

use super::invariants::{c4, check_scene, contains, scene};
use super::*;

/// Nested boundaries, an empty one, boundary↔member and member→boundary
/// relations, a `BiRel`, a self-relation, an unknown endpoint, a reversed
/// pair and a `Rel_U`.
const E1: &str = "C4Container
Person(u, \"User\")
System_Boundary(outer, \"Outer\") {
  Container_Boundary(inner, \"Inner\") {
    Container(a, \"A\", \"Rust\", \"a very long description that should wrap over several lines nicely\")
    Container(b, \"B\")
  }
  Container(c, \"C\")
  Boundary(empty, \"Empty\")
}
Container(d, \"D\")
Rel(outer, a, \"boundary to member\")
Rel(u, inner, \"to boundary\")
BiRel(a, d, \"bi\")
Rel(a, a, \"self\")
Rel(b, ghost, \"unknown\")
Rel(c, a, \"c-a\")
Rel(d, c, \"d-c\")
Rel(c, d, \"c-d back\")
Rel_U(b, u, \"up\")
";

/// An opposite-direction pair across a boundary (A1/B2).
const E2: &str = "C4Container
Container_Boundary(z, \"Zone\") {
  Container(a, \"A\")
  Container(b, \"B\")
}
Container(d, \"D\")
Rel(a, d, \"a-d\")
Rel(d, a, \"d-a\")
Rel(a, b, \"a-b\")
";

const E3: &str = "C4Container
Person(u, \"U\")
Container_Boundary(z, \"Zone\") {
  Container(a, \"A\")
  Container(b, \"B\")
}
Container(d, \"D\")
Rel(u, a, \"u-a\")
Rel(a, d, \"a-d\")
Rel(b, u, \"b-u back\")
";

/// Boundary→own member and a self-relation (A6/A7).
const E4: &str = "C4Container
Container_Boundary(z, \"Zone\") {
  Container(a, \"A\")
}
Rel(z, a, \"z-a\")
Rel(a, a, \"self\")
";

/// A member reusing its boundary's alias (B1).
const E5: &str = "C4Container
Container(y, \"Y\")
Container_Boundary(x, \"X\") {
  Container(x, \"dup\")
}
Rel(y, x, \"r\")
";

fn both_modes(src: &str) -> [C4Diagram; 3] {
    [
        c4(src),
        c4(&format!("{src}LAYOUT_TOP_DOWN()\n")),
        c4(&format!("{src}LAYOUT_LEFT_RIGHT()\n")),
    ]
}

#[test]
fn repros_hold_every_invariant_in_every_mode() {
    for src in [E1, E2, E3, E4, E5] {
        for d in both_modes(src) {
            check_scene(&d);
        }
    }
}

#[test]
fn reversed_pair_across_a_boundary_stays_out_of_its_members() {
    // A1/B2: both directions between `a` (in z) and `d` used to run through
    // A and B, ~811px for a 248px gap.
    for d in both_modes(E2) {
        let s = scene(&d);
        for p in s
            .rels
            .iter()
            .filter(|p| p.rel.from == "d" || p.rel.to == "d")
        {
            let len: f64 = p
                .line
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum();
            let (a, b) = (p.line[0], p.line[p.line.len() - 1]);
            let direct = (b.0 - a.0).hypot(b.1 - a.1);
            assert!(
                len < 1.6 * direct + 60.0,
                "{} -> {}: {len:.0} vs {direct:.0}",
                p.rel.from,
                p.rel.to
            );
        }
    }
}

#[test]
fn self_relation_is_a_loop_with_its_label_off_the_shape() {
    // A6: it used to be a zero-length path at the centre, label on the shape.
    for d in both_modes(E4) {
        let s = scene(&d);
        let plan = s
            .rels
            .iter()
            .find(|p| p.rel.from == p.rel.to)
            .expect("self");
        assert!(plan.curved && plan.path.len() > 2);
        let a = rect_of(&s.placement, "a");
        assert!(plan
            .line
            .iter()
            .all(|q| !contains(a, (q.0, q.1, 0.0, 0.0)) || on_edge(a, *q)));
        let label = plan.label_rect().expect("labelled");
        assert_eq!(crate::svg::label_layout::rect_overlap_area(label, a), 0.0);
    }
}

fn on_edge(r: Rect, p: (f64, f64)) -> bool {
    let near = |v: f64, e: f64| (v - e).abs() < 1.0;
    near(p.0, r.0) || near(p.0, r.0 + r.2) || near(p.1, r.1) || near(p.1, r.1 + r.3)
}

#[test]
fn boundary_to_own_member_runs_from_the_frame_edge() {
    // A7: it used to be an unclipped centre-to-centre line inside the shape.
    for d in both_modes(E4) {
        let s = scene(&d);
        let plan = s.rels.iter().find(|p| p.rel.from == "z").expect("z-a");
        let (z, a) = (rect_of(&s.placement, "z"), rect_of(&s.placement, "a"));
        let start = plan.path[0];
        assert!(on_edge(z, start), "starts on the frame: {start:?} vs {z:?}");
        assert!(on_edge(a, plan.path[plan.path.len() - 1]));
    }
}

#[test]
fn duplicate_alias_terminates_and_the_first_declaration_wins() {
    // B1: a member reusing its boundary's alias made graph mode loop
    // forever. Render on a worker thread so a regression fails instead of
    // hanging the suite.
    for src in [E5.to_string(), format!("{E5}LAYOUT_TOP_DOWN()\n")] {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let d = c4(&src);
            let s = scene(&d);
            let end = s.rels.first().map(|p| p.path[p.path.len() - 1]);
            let frame = s.placement.boundaries.first().map(|b| b.rect);
            let _ = tx.send((end, frame));
        });
        let (end, frame) = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("render finished");
        let (end, frame) = (end.expect("relation"), frame.expect("boundary"));
        // `Rel(y, x)` targets the boundary `x`, declared before its member.
        assert!(on_edge(frame, end), "{end:?} on {frame:?}");
    }
}

#[test]
fn rel_style_matches_a_rel_back_as_written() {
    // Gallery sample 08: the style names the pair in source order.
    let src = "C4Context\nPerson(user, \"U\")\nSystem(top, \"T\")\n\
        Rel_Back(user, top, \"Notified by\")\n\
        UpdateRelStyle(user, top, $textColor=\"#d90429\", $lineColor=\"#d90429\")\n";
    let svg = crate::render(src).expect("renders");
    assert!(svg.contains("stroke=\"#d90429\""), "line colour applied");
    assert!(svg.contains("fill=\"#d90429\""), "label colour applied");
}

#[test]
fn grid_ends_on_one_shape_keep_apart() {
    // r_grid8: "seven" and "six" used to meet at Beta's corner.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/c4/r_grid8.mmd"
    ));
    let d = super::invariants::c4(src);
    let s = super::invariants::scene(&d);
    let ends: Vec<(&str, (f64, f64))> = s
        .rels
        .iter()
        .flat_map(|p| {
            [
                (p.rel.from.as_str(), p.path[0]),
                (p.rel.to.as_str(), p.path[p.path.len() - 1]),
            ]
        })
        .collect();
    for (i, (a, p)) in ends.iter().enumerate() {
        for (b, q) in &ends[i + 1..] {
            if a == b {
                let gap = (p.0 - q.0).hypot(p.1 - q.1);
                assert!(gap >= 11.9, "two ends on {a} {gap:.1}px apart: {p:?} {q:?}");
            }
        }
    }
}
