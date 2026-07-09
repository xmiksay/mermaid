use std::f64::consts::{FRAC_PI_2, PI};

use super::draw::{icon_name, is_dark};
use super::layout::build;
use super::{render, RING_GAP};

use crate::parse::{MindmapDiagram, MindmapNode, MindmapShape};
use crate::svg::theme::Theme;

#[test]
fn produces_svg() {
    let d = MindmapDiagram {
        class_defs: Default::default(),
        root: Some(MindmapNode {
            text: "root".into(),
            shape: MindmapShape::Circle,
            icon: None,
            classes: vec![],
            children: vec![MindmapNode {
                text: "A".into(),
                shape: MindmapShape::Rounded,
                icon: Some("fa fa-book".into()),
                classes: vec![],
                children: vec![],
            }],
        }),
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">root<"));
    assert!(svg.contains(">A<"));
    // The raw Font Awesome class string must not leak into the output as text.
    assert!(!svg.contains("fa fa-book"));
    assert!(!svg.contains("fa-book"));
}

#[test]
fn radial_layout_fans_children_around_root() {
    let leaf = |t: &str| MindmapNode {
        text: t.into(),
        shape: MindmapShape::Default,
        icon: None,
        classes: vec![],
        children: vec![],
    };
    let root = MindmapNode {
        text: "root".into(),
        shape: MindmapShape::Circle,
        icon: None,
        classes: vec![],
        children: vec![leaf("A"), leaf("B"), leaf("C"), leaf("D")],
    };
    let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0);
    // The root sits at the origin; every child sits on the first ring.
    assert_eq!((laid.x, laid.y), (0.0, 0.0));
    for c in &laid.children {
        let r = (c.x * c.x + c.y * c.y).sqrt();
        assert!((r - RING_GAP).abs() < 1e-6, "child off the first ring");
    }
    // Four evenly-fanned branches must not all sit on one side of the root:
    // some grow to the right (x>0) and some to the left (x<0).
    assert!(laid.children.iter().any(|c| c.x > 1.0));
    assert!(laid.children.iter().any(|c| c.x < -1.0));
    // First-level branches carry their own section index.
    let sections: Vec<i32> = laid.children.iter().map(|c| c.section).collect();
    assert_eq!(sections, vec![0, 1, 2, 3]);
}

#[test]
fn descendants_inherit_branch_section() {
    let d = match crate::parse::parse("mindmap\nroot((R))\n  Branch\n    Child\n      Grandchild\n")
        .unwrap()
    {
        crate::parse::Diagram::Mindmap(m) => m,
        _ => panic!("not mindmap"),
    };
    let root = d.root.clone().unwrap();
    let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0);
    let branch = &laid.children[0];
    assert_eq!(branch.section, 0);
    assert_eq!(branch.children[0].section, 0);
    assert_eq!(branch.children[0].children[0].section, 0);
}

#[test]
fn icon_attaches_to_annotated_node() {
    // The book icon annotates `Mindmap`, the clock annotates `Gantt`; each
    // glyph must render inside its own node, not float onto a sibling.
    let d = match crate::parse::parse(
        "mindmap\nroot((R))\n  Diagrams\n    Mindmap\n      ::icon(fa fa-book)\n    Gantt\n      ::icon(fa fa-clock)\n",
    )
    .unwrap()
    {
        crate::parse::Diagram::Mindmap(m) => m,
        _ => panic!("not mindmap"),
    };
    let diagrams = &d.root.as_ref().unwrap().children[0];
    assert_eq!(diagrams.children[0].text, "Mindmap");
    assert_eq!(diagrams.children[0].icon.as_deref(), Some("fa fa-book"));
    assert_eq!(diagrams.children[1].text, "Gantt");
    assert_eq!(diagrams.children[1].icon.as_deref(), Some("fa fa-clock"));
    // Both glyphs are drawn, and no raw class string leaks.
    let svg = render(&d, &Theme::default());
    assert!(!svg.contains("fa-book"));
    assert!(!svg.contains("fa-clock"));
}

#[test]
fn branch_nodes_are_filled_from_the_scale() {
    let d = match crate::parse::parse("mindmap\nroot((R))\n  First\n  Second\n").unwrap() {
        crate::parse::Diagram::Mindmap(m) => m,
        _ => panic!("not mindmap"),
    };
    let theme = Theme::default();
    let svg = render(&d, &theme);
    // Branch nodes are filled rounded rects one slot past the generic scale
    // (section 0 = cScale1, section 1 = cScale2 — the upstream rotation), not
    // bare underlined text.
    assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(1))));
    assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(2))));
    assert!(svg.contains("rx=\"8\""));
    // The root disc is the theme's bright-blue primary lane color, not the
    // old dark-purple disc; nodes are borderless (stroke none) + shadowed.
    assert!(svg.contains(&format!("fill=\"{}\"", theme.git_color(0))));
    assert!(svg.contains("stroke=\"none\""));
    assert!(svg.contains("filter=\"url(#mm-shadow)\""));
}

#[test]
fn classdef_recolors_node() {
    use crate::parse::parse;
    let d = match parse(
        "mindmap\nroot(Root)\n  A[Node]\n  :::hot\nclassDef hot fill:#abc123,color:#ffffff\n",
    )
    .unwrap()
    {
        crate::parse::Diagram::Mindmap(m) => m,
        _ => panic!("not mindmap"),
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#abc123\""));
    assert!(svg.contains("fill=\"#ffffff\""));
}

#[test]
fn icon_name_extraction() {
    assert_eq!(icon_name("fa fa-book"), "book");
    assert_eq!(icon_name("fab fa-github"), "github");
    assert_eq!(icon_name("book"), "book");
    assert_eq!(icon_name(""), "");
}

#[test]
fn color_helpers() {
    assert!(is_dark("#000000"));
    assert!(!is_dark("#ffffff"));
    assert!(!is_dark("#B9B9FF"));
    // The bright-blue root (git0) is dark enough to warrant white label text.
    assert!(is_dark("#6D6DFF"));
}
