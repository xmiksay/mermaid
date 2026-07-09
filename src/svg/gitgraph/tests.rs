use super::*;
use crate::parse::CommitKind;

#[test]
fn renders_simple_graph() {
    let d = GitGraphDiagram {
        title: Some("git".into()),
        direction: GitDirection::LeftRight,
        events: vec![
            GitEvent::Commit {
                id: None,
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Commit {
                id: None,
                tags: vec!["v1".into()],
                kind: CommitKind::Highlight,
            },
            GitEvent::Branch {
                name: "dev".into(),
                order: None,
            },
            GitEvent::Commit {
                id: None,
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">git<"));
    assert!(svg.contains(">main<"));
    assert!(svg.contains(">dev<"));
}

fn linear(direction: GitDirection) -> GitGraphDiagram {
    GitGraphDiagram {
        title: None,
        direction,
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Commit {
                id: Some("b".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
        ],
        ..Default::default()
    }
}

#[test]
fn bt_flips_the_commit_axis() {
    // BT must not render identically to TB (issue #61, bug 4).
    let tb = render(&linear(GitDirection::TopDown), &Theme::default());
    let bt = render(&linear(GitDirection::BottomTop), &Theme::default());
    assert_ne!(tb, bt);
}

#[test]
fn branch_order_reorders_lanes() {
    // `low` is declared *after* `high` but its smaller order must place it
    // in the earlier lane (issue #61, bug 2).
    let d = GitGraphDiagram {
        title: None,
        direction: GitDirection::TopDown,
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Branch {
                name: "high".into(),
                order: Some(5),
            },
            GitEvent::Branch {
                name: "low".into(),
                order: Some(1),
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    let low_x = lane_x_before(&svg, svg.find(">low<").unwrap());
    let high_x = lane_x_before(&svg, svg.find(">high<").unwrap());
    assert!(low_x < high_x, "lower order should claim the earlier lane");
}

#[test]
fn main_branch_name_honored() {
    let d = GitGraphDiagram {
        events: vec![GitEvent::Commit {
            id: None,
            tags: Vec::new(),
            kind: CommitKind::Normal,
        }],
        config: crate::parse::GitGraphConfig {
            main_branch_name: "master".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">master<"));
    assert!(!svg.contains(">main<"));
}

#[test]
fn show_branches_and_commit_label_suppressed() {
    let d = GitGraphDiagram {
        events: vec![GitEvent::Commit {
            id: Some("only".into()),
            tags: Vec::new(),
            kind: CommitKind::Normal,
        }],
        config: crate::parse::GitGraphConfig {
            show_branches: false,
            show_commit_label: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // No branch label and no commit id label.
    assert!(!svg.contains(">main<"));
    assert!(!svg.contains(">only<"));
}

#[test]
fn merge_and_cherry_pick_use_distinct_glyphs() {
    // A merge and a cherry-pick must not reuse the highlight/reverse glyphs.
    let d = GitGraphDiagram {
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Branch {
                name: "dev".into(),
                order: None,
            },
            GitEvent::Commit {
                id: Some("b".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Checkout {
                name: "main".into(),
            },
            GitEvent::Merge {
                from: "dev".into(),
                id: Some("m".into()),
                tags: Vec::new(),
                kind: CommitKind::Merge,
            },
            GitEvent::CherryPick {
                commit_id: "b".into(),
                parent: None,
                tag: Some("cp".into()),
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Cherry-pick tag renders inside a tag shape (plain text, no brackets).
    assert!(svg.contains(">cp<"));
    // The cherry glyph draws small r="2.5" circles; the merge glyph an
    // inner r="5" ring — neither is a rotated square (reverse glyph).
    assert!(svg.contains("r=\"2.5\""));
    assert!(svg.contains("r=\"5\""));
    assert!(!svg.contains("rotate(45"));
}

#[test]
fn merge_type_override_draws_highlight_glyph() {
    // `merge dev type: HIGHLIGHT` must not draw the merge glyph.
    let d = GitGraphDiagram {
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Branch {
                name: "dev".into(),
                order: None,
            },
            GitEvent::Commit {
                id: Some("b".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Checkout {
                name: "main".into(),
            },
            GitEvent::Merge {
                from: "dev".into(),
                id: Some("m".into()),
                tags: Vec::new(),
                kind: CommitKind::Highlight,
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // The highlight glyph is an r="12" circle (COMMIT_R + 2); the merge
    // glyph's inner ring is r="5" — absent here.
    assert!(svg.contains("r=\"12\""));
    assert!(!svg.contains("r=\"5\""));
}

#[test]
fn multiple_tags_all_render() {
    let d = GitGraphDiagram {
        events: vec![GitEvent::Commit {
            id: Some("a".into()),
            tags: vec!["v1".into(), "v2".into()],
            kind: CommitKind::Normal,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">v1<"));
    assert!(svg.contains(">v2<"));
}

#[test]
fn main_branch_order_positions_main_among_lanes() {
    // With mainBranchOrder=2, `main` (order 2) sits after `dev` (insertion
    // lane 1) instead of claiming the first lane.
    let d = GitGraphDiagram {
        direction: GitDirection::TopDown,
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Branch {
                name: "dev".into(),
                order: None,
            },
        ],
        config: crate::parse::GitGraphConfig {
            main_branch_order: Some(2),
            ..Default::default()
        },
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    let main_x = lane_x_before(&svg, svg.find(">main<").unwrap());
    let dev_x = lane_x_before(&svg, svg.find(">dev<").unwrap());
    assert!(dev_x < main_x, "dev should claim the earlier lane");
}

#[test]
fn branch_labels_render_as_filled_pills() {
    let d = GitGraphDiagram {
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Branch {
                name: "dev".into(),
                order: None,
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // A rounded pill rect precedes the "main" label text.
    let label = svg.find(">main<").unwrap();
    let rect = svg[..label].rfind("<rect").unwrap();
    assert!(svg[rect..label].contains("rx=\"10\""));
}

#[test]
fn tags_render_as_tag_shapes() {
    let d = GitGraphDiagram {
        events: vec![GitEvent::Commit {
            id: Some("a".into()),
            tags: vec!["v1.0".into()],
            kind: CommitKind::Normal,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Tag body path + yellow luggage-tag fill, no bracketed text.
    assert!(svg.contains(TAG_FILL));
    assert!(svg.contains(">v1.0<"));
    assert!(!svg.contains("[v1.0]"));
}

#[test]
fn thick_trunk_and_trailing_dash() {
    let d = GitGraphDiagram {
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Commit {
                id: Some("b".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // The branch trunk is thick, and a dotted continuation trails the last
    // commit.
    assert!(svg.contains(&format!("stroke-width=\"{}\"", fnum(LINE_W))));
    assert!(svg.contains("stroke-dasharray=\"4 4\""));
}

#[test]
fn auto_ids_are_seq_hash_style() {
    // Auto ids look like upstream's `0-<hash>`, not `c1`.
    let d = GitGraphDiagram {
        events: vec![GitEvent::Commit {
            id: None,
            tags: Vec::new(),
            kind: CommitKind::Normal,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">0-"));
    assert!(!svg.contains(">c1<"));
}

#[test]
fn merge_join_is_a_rounded_elbow() {
    // Cross-lane joins use a quadratic-cornered elbow, not an S-curve.
    let d = GitGraphDiagram {
        events: vec![
            GitEvent::Commit {
                id: Some("a".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
            GitEvent::Branch {
                name: "dev".into(),
                order: None,
            },
            GitEvent::Commit {
                id: Some("b".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // A rounded elbow path uses an `L…Q…L` shape (no cubic `C`).
    assert!(svg.contains("Q"));
}

/// Reads the `x="…"` of the `<text>` element ending just before byte `end`.
fn lane_x_before(svg: &str, end: usize) -> f64 {
    let text_start = svg[..end].rfind("<text").unwrap();
    let seg = &svg[text_start..end];
    let x_pos = seg.find("x=\"").unwrap() + 3;
    let rest = &seg[x_pos..];
    let x_end = rest.find('"').unwrap();
    rest[..x_end].parse().unwrap()
}
