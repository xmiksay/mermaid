//! Graph (sugiyama) mode, exercised end-to-end on the #396 issue sample.

use super::invariants::{c4, check_frames, check_scene, contains, scene};
use super::*;

const ISSUE_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/samples/c4/11-container-graph-layout.mmd"
));

fn chain(n: usize, header: &str) -> String {
    let mut src = format!("C4Container\n{header}\n");
    for i in 0..n {
        src.push_str(&format!("Container(c{i}, \"C{i}\", \"T\", \"D\")\n"));
    }
    for i in 1..n {
        src.push_str(&format!("Rel(c{}, c{i}, \"r{i}\")\n", i - 1));
    }
    src
}

#[test]
fn mode_follows_relation_count_and_layout_statements() {
    assert!(
        !uses_graph_layout(&c4(&chain(9, ""))),
        "8 relations stay on the grid"
    );
    assert!(uses_graph_layout(&c4(&chain(10, ""))), "9 relations switch");
    assert!(uses_graph_layout(&c4(&chain(2, "LAYOUT_TOP_DOWN()"))));
    // Only relations that get drawn count: self-relations and relations to
    // undefined aliases don't push an 8-relation chain over the threshold.
    let padded = format!(
        "{}Rel(c0, c0, \"self\")\nRel(c0, ghost, \"to nothing\")\n",
        chain(9, "")
    );
    assert!(!uses_graph_layout(&c4(&padded)));
    assert!(uses_graph_layout(&c4(&chain(2, "LAYOUT_LEFT_RIGHT()"))));
    let sample = c4(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/samples/c4.mmd"
    )));
    assert!(
        !uses_graph_layout(&sample),
        "samples/c4.mmd keeps the grid (#327/#370)"
    );
    assert!(!scene(&sample).placement.curved);
}

#[test]
fn layout_direction_orients_the_layers() {
    let td = c4(&chain(3, "LAYOUT_TOP_DOWN()"));
    let lr = c4(&chain(3, "LAYOUT_LEFT_RIGHT()"));
    let (td, lr) = (scene(&td), scene(&lr));
    let (a, b) = (rect_of(&td.placement, "c0"), rect_of(&td.placement, "c1"));
    assert!(b.1 >= a.1 + a.3, "top-down: c1 below c0");
    let (a, b) = (rect_of(&lr.placement, "c0"), rect_of(&lr.placement, "c1"));
    assert!(b.0 >= a.0 + a.2, "left-right: c1 right of c0");
    assert!((a.1 - b.1).abs() < 1.0 || (a.1 + a.3 / 2.0 - b.1 - b.3 / 2.0).abs() < 1.0);
}

#[test]
fn issue_sample_uses_curved_graph_layout() {
    let d = c4(ISSUE_SAMPLE);
    assert!(uses_graph_layout(&d));
    let s = scene(&d);
    assert!(s.placement.curved);
    assert_eq!(s.rels.len(), d.relations.len());
    assert!(
        render(&d, &Theme::default()).contains(" C"),
        "splines, not lines"
    );
}

#[test]
fn issue_sample_geometry_top_down() {
    check_scene(&c4(ISSUE_SAMPLE));
}

#[test]
fn issue_sample_geometry_left_right() {
    let d = c4(&format!("{ISSUE_SAMPLE}\nLAYOUT_LEFT_RIGHT()\n"));
    assert!(scene(&d).placement.narrow_labels);
    check_scene(&d);
}

#[test]
fn issue_sample_main_flow_runs_down() {
    // A3: Workflow, the main source, sits above the core services it feeds
    // (three relations down vs two back up), not second to last.
    let d = c4(ISSUE_SAMPLE);
    let p = scene(&d).placement;
    let (flow, zone1) = (rect_of(&p, "flow"), rect_of(&p, "zone1"));
    assert!(flow.1 + flow.3 <= zone1.1, "{flow:?} vs {zone1:?}");
    // The source chain Events -> Detector -> Workflow runs downward too,
    // even though reversing Detector -> Workflow alone would be one
    // reversal cheaper (zone-1 feeds Detector twice).
    let (feeds, detect) = (rect_of(&p, "feeds"), rect_of(&p, "detect"));
    assert!(feeds.1 + feeds.3 <= detect.1, "{feeds:?} vs {detect:?}");
    assert!(detect.1 + detect.3 <= flow.1, "{detect:?} vs {flow:?}");
    // Sources feeding Workflow sit just above it, not on the top layer of
    // the whole diagram far away.
    let monitor = rect_of(&p, "monitor");
    assert!(
        flow.1 - (monitor.1 + monitor.3) < 300.0,
        "{monitor:?} vs {flow:?}"
    );
}

#[test]
fn issue_sample_ends_on_one_face_are_spread() {
    // A4/A8: no two arrowheads (or line starts) meet within 12px, in either
    // direction.
    for src in [
        ISSUE_SAMPLE.to_string(),
        format!("{ISSUE_SAMPLE}\nLAYOUT_LEFT_RIGHT()\n"),
    ] {
        let d = c4(&src);
        let s = scene(&d);
        let ends: Vec<(f64, f64)> = s
            .rels
            .iter()
            .flat_map(|p| [p.path[0], p.path[p.path.len() - 1]])
            .collect();
        for (i, a) in ends.iter().enumerate() {
            for b in &ends[i + 1..] {
                assert!((a.0 - b.0).hypot(a.1 - b.1) >= 12.0, "ends {a:?} and {b:?}");
            }
        }
    }
}

#[test]
fn issue_sample_lines_share_no_long_stretch() {
    // A4: relations that leave one boundary towards another each get their
    // own dummy chain, so no two lines run together for long.
    let d = c4(ISSUE_SAMPLE);
    let s = scene(&d);
    let near = |p: (f64, f64), line: &[(f64, f64)]| {
        line.iter().any(|q| (p.0 - q.0).hypot(p.1 - q.1) < 3.0)
    };
    for (i, a) in s.rels.iter().enumerate() {
        for b in &s.rels[i + 1..] {
            let shared: f64 = a
                .line
                .windows(2)
                .filter(|w| near(w[0], &b.line) && near(w[1], &b.line))
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum();
            assert!(
                shared < 60.0,
                "{}->{} and {}->{} share {shared:.0}px",
                a.rel.from,
                a.rel.to,
                b.rel.from,
                b.rel.to
            );
        }
    }
}

#[test]
fn nested_boundaries_stay_nested_in_both_directions() {
    let src = "C4Container\n\
        Person(u, \"User\")\n\
        System_Boundary(outer, \"Outer\") {\n\
          Container(a, \"A\", \"T\", \"D\")\n\
          Container_Boundary(inner, \"Inner\") {\n\
            Container(b, \"B\", \"T\", \"D\")\n\
            ContainerDb(c, \"C\", \"SQL\", \"D\")\n\
          }\n\
        }\n\
        System_Ext(x, \"X\")\n\
        Rel(u, a, \"uses\")\nRel(a, b, \"calls\")\nRel(b, c, \"reads\")\n\
        Rel(c, x, \"syncs\")\nRel(u, b, \"direct\")\n";
    for dir in ["LAYOUT_TOP_DOWN()", "LAYOUT_LEFT_RIGHT()"] {
        let d = c4(&format!("{src}{dir}\n"));
        let s = scene(&d);
        check_frames(&d, &s.placement);
        check_scene(&d);
        assert!(contains(
            rect_of(&s.placement, "outer"),
            rect_of(&s.placement, "inner")
        ));
    }
}
