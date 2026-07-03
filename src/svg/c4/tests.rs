use super::*;
use crate::parse::{C4Kind, C4RelDirection, C4Relation};

fn person(alias: &str, label: &str) -> C4Element {
    C4Element {
        kind: C4ElementKind::Person,
        alias: alias.into(),
        label: label.into(),
        descr: None,
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        external: false,
        boundary_alias: None,
        boundary_label: None,
        boundary_kind: None,
        members: vec![],
    }
}

fn boundary(alias: &str, label: &str, kind: C4BoundaryKind, members: Vec<C4Element>) -> C4Element {
    C4Element {
        kind: C4ElementKind::System,
        alias: alias.into(),
        label: label.into(),
        descr: None,
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        external: false,
        boundary_alias: None,
        boundary_label: None,
        boundary_kind: Some(kind),
        members,
    }
}

#[test]
fn produces_svg() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: Some("Sys".into()),
        elements: vec![person("u", "User")],
        relations: vec![],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">User<"));
    assert!(svg.contains(">Sys<"));
}

fn container(alias: &str, label: &str, members: Vec<C4Element>) -> C4Element {
    boundary(alias, label, C4BoundaryKind::Deployment, members)
}

/// Regression for #5: with a title present, the topmost boundary header must
/// not overlap the title/subtitle text. The subtitle baseline is at PAD+38;
/// the boundary rect top must sit below it.
#[test]
fn boundary_clears_title() {
    let d = C4Diagram {
        kind: C4Kind::Deployment,
        title: Some("Deployment".into()),
        elements: vec![container(
            "app06",
            "app06",
            vec![person("uportal", "portal")],
        )],
        relations: vec![],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());

    // Boundary rects carry the `rx="2.5"` corner (elements use rx="6"). Find
    // each and check its `y` clears the subtitle baseline.
    let subtitle_baseline = PAD + 38.0;
    let mut checked = false;
    for chunk in svg.split("<rect").skip(1) {
        if !chunk.contains("rx=\"2.5\"") {
            continue;
        }
        let y = extract_attr(chunk, "y=\"").expect("boundary rect has y");
        assert!(
            y > subtitle_baseline,
            "boundary top {y} overlaps title (subtitle baseline {subtitle_baseline})"
        );
        checked = true;
    }
    assert!(checked, "expected at least one boundary rect");
}

fn extract_attr(s: &str, key: &str) -> Option<f64> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let end = rest.find('"')?;
    rest[..end].parse().ok()
}

#[test]
fn arrow_marker_present() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![person("a", "A"), person("b", "B")],
        relations: vec![C4Relation {
            from: "a".into(),
            to: "b".into(),
            label: "uses".into(),
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("c4-arrow"));
    assert!(svg.contains("marker-end=\"url(#c4-arrow)\""));
    assert!(!svg.contains("marker-start=\"url(#c4-arrow)\""));
}

#[test]
fn bidirectional_has_both_markers() {
    let d = C4Diagram {
        kind: C4Kind::Container,
        title: None,
        elements: vec![person("a", "A"), person("b", "B")],
        relations: vec![C4Relation {
            from: "a".into(),
            to: "b".into(),
            label: "syncs".into(),
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: true,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("marker-start=\"url(#c4-arrow)\""));
    assert!(svg.contains("marker-end=\"url(#c4-arrow)\""));
}

#[test]
fn relations_are_solid() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![person("a", "A"), person("b", "B")],
        relations: vec![C4Relation {
            from: "a".into(),
            to: "b".into(),
            label: "uses".into(),
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // The connector path must not be dashed (only the boundary outline is).
    assert!(!svg.contains("stroke-dasharray=\"5 4\""));
}

#[test]
fn deployment_node_boundary_is_solid() {
    let d = C4Diagram {
        kind: C4Kind::Deployment,
        title: None,
        elements: vec![boundary(
            "dn",
            "Server",
            C4BoundaryKind::Deployment,
            vec![person("a", "A"), person("b", "B")],
        )],
        relations: vec![],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Solid border: #444444, width 1, no dasharray on the boundary rect.
    assert!(svg.contains("stroke=\"#444444\" stroke-width=\"1\" rx=\"2.5\""));
    assert!(!svg.contains("stroke-dasharray"));
    assert!(svg.contains(">[Deployment Node]<"));
}

#[test]
fn generic_boundary_is_dashed_7_7() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![boundary(
            "b",
            "Group",
            C4BoundaryKind::System,
            vec![person("a", "A"), person("b", "B")],
        )],
        relations: vec![],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(
        "stroke=\"#444444\" stroke-width=\"1\" rx=\"2.5\" ry=\"2.5\" stroke-dasharray=\"7 7\""
    ));
}

#[test]
fn rel_is_curved_and_unbacked() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![person("a", "A"), person("b", "B")],
        relations: vec![C4Relation {
            from: "a".into(),
            to: "b".into(),
            label: "uses".into(),
            technology: Some("HTTPS".into()),
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Quadratic Bézier, #444444, width 1.
    assert!(svg.contains(" Q"));
    assert!(svg.contains("stroke=\"#444444\" stroke-width=\"1\""));
    // No translucent label background rect.
    assert!(!svg.contains("fill-opacity=\"0.5\""));
    // techn rendered italic as [HTTPS].
    assert!(svg.contains(">[HTTPS]<"));
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

#[test]
fn sibling_boundaries_do_not_overlap() {
    // Four Deployment_Node boundaries (as in the CyberScore repro), each
    // holding a couple of shapes. None of the frames may overlap.
    let elements = vec![
        boundary(
            "app17",
            "app17",
            C4BoundaryKind::Deployment,
            vec![person("a1", "A1"), person("a2", "A2")],
        ),
        boundary(
            "app06",
            "app06",
            C4BoundaryKind::Deployment,
            vec![person("b1", "B1"), person("b2", "B2")],
        ),
        boundary(
            "app14",
            "app14",
            C4BoundaryKind::Deployment,
            vec![person("c1", "C1")],
        ),
        boundary(
            "app16",
            "app16",
            C4BoundaryKind::Deployment,
            vec![person("d1", "D1")],
        ),
    ];

    let (nodes, _, _) = flow_layout(&elements, SHAPE_IN_ROW, BOUNDARY_IN_ROW, 14.0);
    let mut pos = HashMap::new();
    let mut boundaries = Vec::new();
    let mut leaves = Vec::new();
    place_absolute(&nodes, PAD, PAD, &mut pos, &mut boundaries, &mut leaves);

    assert_eq!(boundaries.len(), 4);
    for (i, a) in boundaries.iter().enumerate() {
        for b in &boundaries[i + 1..] {
            let ra = (a.x, a.y, a.w, a.h);
            let rb = (b.x, b.y, b.w, b.h);
            assert!(
                !overlaps(ra, rb),
                "boundary frames overlap: {ra:?} vs {rb:?}"
            );
        }
    }
}

#[test]
fn boundary_contains_its_members() {
    let elements = vec![boundary(
        "app",
        "app",
        C4BoundaryKind::Deployment,
        vec![person("x", "X"), person("y", "Y")],
    )];
    let (nodes, _, _) = flow_layout(&elements, SHAPE_IN_ROW, BOUNDARY_IN_ROW, 14.0);
    let mut pos = HashMap::new();
    let mut boundaries = Vec::new();
    let mut leaves = Vec::new();
    place_absolute(&nodes, PAD, PAD, &mut pos, &mut boundaries, &mut leaves);

    let b = &boundaries[0];
    for (_, x, y, w, h) in &leaves {
        assert!(
            *x >= b.x && *x + *w <= b.x + b.w,
            "member escapes boundary x"
        );
        assert!(
            *y >= b.y && *y + *h <= b.y + b.h,
            "member escapes boundary y"
        );
    }
}

#[test]
fn element_style_override_applies_colors() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![C4Element {
            kind: C4ElementKind::System,
            alias: "s".into(),
            label: "Sys".into(),
            descr: None,
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: None,
            members: vec![],
        }],
        relations: vec![],
        element_styles: vec![C4ElementStyle {
            alias: "s".into(),
            bg_color: Some("#ABCDEF".into()),
            font_color: None,
            border_color: Some("#123456".into()),
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#ABCDEF\""));
    assert!(svg.contains("stroke=\"#123456\""));
}

#[test]
fn rel_style_override_colors_line_and_label() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![person("a", "A"), person("b", "B")],
        relations: vec![C4Relation {
            from: "a".into(),
            to: "b".into(),
            label: "uses".into(),
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
        }],
        rel_styles: vec![C4RelStyle {
            from: "a".into(),
            to: "b".into(),
            text_color: Some("#00FF00".into()),
            line_color: Some("#FF0000".into()),
            offset_x: None,
            offset_y: None,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("stroke=\"#FF0000\""));
    assert!(svg.contains("fill=\"#00FF00\""));
}

#[test]
fn boundary_style_override_applies_colors() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![boundary(
            "b",
            "Group",
            C4BoundaryKind::System,
            vec![person("a", "A")],
        )],
        relations: vec![],
        boundary_styles: vec![C4ElementStyle {
            alias: "b".into(),
            bg_color: Some("#EEEEEE".into()),
            font_color: None,
            border_color: Some("#333333".into()),
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#EEEEEE\" stroke=\"#333333\""));
}

#[test]
fn layout_config_controls_shapes_per_row() {
    // With shape_in_row = 1 the two shapes stack vertically; default (4)
    // would place them on the same row. Verify the override changes layout.
    let elements = vec![person("a", "A"), person("b", "B")];
    let (nodes, _, _) = flow_layout(&elements, 1, BOUNDARY_IN_ROW, 14.0);
    assert_eq!(nodes.len(), 2);
    assert!(
        nodes[1].y > nodes[0].y,
        "second shape should wrap to the next row"
    );
    assert_eq!(nodes[0].x, nodes[1].x, "wrapped shapes share the left edge");
}
