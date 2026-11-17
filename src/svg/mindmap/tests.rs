use std::f64::consts::{FRAC_PI_2, PI};

use super::draw::{icon_name, is_dark};
use super::layout::{bounds, build};
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
    let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0, 0.0);
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
    let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0, 0.0);
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
fn bounds_enclose_a_wide_circle_nodes_full_radius() {
    // A `((long label))` circle sizes its rendered radius from
    // `max(w, h) / 2` (see `draw::draw_shape`), which for a long label is
    // driven by the (wide) text width, not the (fixed) node height — a
    // circle whose radius comes from `w` reaches further vertically than a
    // plain box's `h / 2` half-height would suggest. `bounds` must use that
    // same radius on both axes or the circle's top/bottom edge gets clipped
    // by the canvas (issue: circle node cropped at canvas edge).
    let root = MindmapNode {
        text: "root".into(),
        shape: MindmapShape::Circle,
        icon: None,
        classes: vec![],
        children: vec![MindmapNode {
            text: "A very long circle label that forces a wide radius".into(),
            shape: MindmapShape::Circle,
            icon: None,
            classes: vec![],
            children: vec![],
        }],
    };
    let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0, 0.0);
    let child = &laid.children[0];
    let true_radius = (child.w / 2.0).max(child.h / 2.0);
    assert!(
        true_radius > child.h / 2.0,
        "test setup must exercise the wide-circle case (w > h)"
    );
    let (min_x, min_y, max_x, max_y) = bounds(&laid);
    assert!(
        max_y >= child.y + true_radius - 1e-6,
        "bounds bottom edge ({max_y}) must reach the circle's full radius ({})",
        child.y + true_radius
    );
    assert!(
        min_y <= child.y - true_radius + 1e-6,
        "bounds top edge ({min_y}) must reach the circle's full radius ({})",
        child.y - true_radius
    );
    assert!(max_x >= child.x + true_radius - 1e-6);
    assert!(min_x <= child.x - true_radius + 1e-6);
}

#[test]
fn color_helpers() {
    assert!(is_dark("#000000"));
    assert!(!is_dark("#ffffff"));
    assert!(!is_dark("#B9B9FF"));
    // The bright-blue root (git0) is dark enough to warrant white label text.
    assert!(is_dark("#6D6DFF"));
}

#[test]
fn cloud_and_bang_have_their_own_outlines() {
    // `)cloud(` and `))bang((` used to render as a rounded / dashed rect.
    let svg = crate::render("mindmap\n  root((Hub))\n    a)Cloudy(\n    b))Boom((\n").unwrap();
    assert!(svg.contains("Cloudy") && svg.contains("Boom"));
    assert!(
        !svg.contains("stroke-dasharray=\"4 2\""),
        "bang is no longer a dashed rect"
    );
    // Cloud: an outline made of arcs; bang: a closed polyline.
    let cloud = super::draw::cloud_path(100.0, 50.0, 60.0, 16.0);
    assert!(cloud.starts_with('M') && cloud.ends_with('Z') && cloud.matches('A').count() >= 8);
    let bang = super::draw::bang_path(100.0, 50.0, 60.0, 16.0);
    assert!(bang.starts_with('M') && bang.ends_with('Z') && bang.matches('L').count() >= 15);
    for p in [&cloud, &bang] {
        assert!(!p.contains("NaN") && !p.contains("inf"));
    }
    // Degenerate boxes stay finite.
    assert!(!super::draw::cloud_path(0.0, 0.0, 0.0, 0.0).contains("NaN"));
    assert!(!super::draw::bang_path(0.0, 0.0, 0.0, 0.0).contains("NaN"));
}
