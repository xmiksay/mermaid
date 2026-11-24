//! Graph-mode internals: route stitching and port-side settling.

use super::pass::Pass;
use super::route::assemble;
use super::tree::{rel_path, RelPath, Tree};
use super::*;

fn local(frame: Span, centers: Vec<Pt>, rel_pts: Vec<(usize, Vec<Pt>)>) -> Local {
    let sizes = vec![(40.0, 40.0); centers.len()];
    Local {
        frame,
        header: None,
        centers,
        sizes,
        rel_pts: rel_pts.into_iter().collect(),
    }
}

/// Root: block B (frame 0..100 × 0..100, centre (50, 50)) above leaf d at
/// (50, 300). Inside B: member a at (50, 70) with a port below the frame.
fn two_levels() -> Vec<Local> {
    vec![
        local(
            Span::new(0.0, 0.0, 100.0, 320.0),
            vec![(50.0, 50.0), (50.0, 300.0)],
            vec![
                (0, vec![(50.0, 50.0), (50.0, 300.0)]),
                (1, vec![(50.0, 300.0), (50.0, 50.0)]),
            ],
        ),
        local(
            Span::new(0.0, 0.0, 100.0, 100.0),
            vec![(50.0, 70.0)],
            vec![
                (0, vec![(50.0, 70.0), (50.0, 150.0)]),
                (1, vec![(50.0, 150.0), (50.0, 70.0)]),
            ],
        ),
    ]
}

#[test]
fn assemble_joins_a_member_to_an_outside_target_at_the_frame() {
    let locals = two_levels();
    let offset = [(0.0, 0.0), (0.0, 0.0)];
    let out = RelPath {
        common: 0,
        src_side: vec![1],
        dst_side: vec![],
    };
    let mut slots = super::route::FrameMarks::default();
    let route = assemble(&out, 0, &locals, &offset, false, &mut slots).expect("route");
    assert_eq!(
        route,
        vec![
            (50.0, 70.0),
            (50.0, 100.0),
            (50.0, 100.0),
            (50.0, 112.0),
            (50.0, 112.0),
            (50.0, 300.0)
        ]
    );
    // The reverse relation enters through the same frame edge.
    let back = RelPath {
        common: 0,
        src_side: vec![],
        dst_side: vec![1],
    };
    let route = assemble(&back, 1, &locals, &offset, false, &mut slots).expect("route");
    assert_eq!(
        route,
        // A second route through the same frame edge crosses it EXIT_GAP
        // further along, never on top of the first.
        vec![
            (50.0, 300.0),
            (62.0, 112.0),
            (62.0, 112.0),
            (62.0, 100.0),
            (62.0, 100.0),
            (50.0, 70.0)
        ]
    );
    // A relation with no piece at some level has no route.
    assert!(assemble(&out, 7, &locals, &offset, false, &mut slots).is_none());
}

fn c4(src: &str) -> C4Diagram {
    match crate::parse::parse(src) {
        Ok(crate::parse::Diagram::C4(d)) => d,
        other => panic!("not a C4 diagram: {other:?}"),
    }
}

const ISSUE_SAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/samples/c4/11-container-graph-layout.mmd"
));

/// The port sides a settled layout was made with are the sides it reveals.
fn assert_settled(src: &str) {
    let d = c4(src);
    let dir = Dir {
        lr: d.layout_direction == Some(C4LayoutDirection::LeftRight),
    };
    let tree = Tree::build(&d.elements);
    let paths: Vec<_> = d.relations.iter().map(|r| rel_path(&tree, r)).collect();
    let pass = Pass {
        tree: &tree,
        paths: &paths,
        rels: &d.relations,
        dir,
        cfg: layout_config(&d.relations, dir),
        font_size: 14.0,
        loops: super::super::rel_loop::loop_room(&d.relations, dir.lr),
        enclosures: super::super::rel_loop::enclosure_room(&d.elements, &d.relations, dir.lr),
    };
    let ((locals, offset), sides, converged) = pass.settle_checked().expect("layout");
    assert!(converged, "port sides did not settle");
    assert_eq!(pass.port_sides(&locals, &offset).0, sides);
}

#[test]
fn port_sides_settle_on_every_fixture() {
    let n1 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/c4/n1_component.mmd"
    ));
    let n2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/c4/n2_deploy.mmd"
    ));
    for src in [ISSUE_SAMPLE, n1, n2] {
        assert_settled(src);
        assert_settled(&format!("{src}\nLAYOUT_LEFT_RIGHT()\n"));
    }
}

#[test]
fn port_sides_face_the_other_end() {
    // `a` (in z) talks to `d` both ways; once settled both of z's ports
    // face d, wherever the layout put it.
    let src = "C4Container\nLAYOUT_TOP_DOWN()\nContainer_Boundary(z, \"Zone\") {\n\
        Container(a, \"A\")\nContainer(b, \"B\")\n}\nContainer(d, \"D\")\n\
        Rel(a, d, \"a-d\")\nRel(d, a, \"d-a\")\nRel(a, b, \"a-b\")\n";
    assert_settled(src);
}
