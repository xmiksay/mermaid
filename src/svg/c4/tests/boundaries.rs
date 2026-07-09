use super::*;
use crate::parse::C4Kind;

fn container(alias: &str, label: &str, members: Vec<C4Element>) -> C4Element {
    boundary(alias, label, C4BoundaryKind::Deployment, members)
}

fn extract_attr(s: &str, key: &str) -> Option<f64> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let end = rest.find('"')?;
    rest[..end].parse().ok()
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
fn boundary_type_overrides_kind_label() {
    let mut node = boundary(
        "n1",
        "Web Server",
        C4BoundaryKind::Deployment,
        vec![person("a", "A")],
    );
    node.boundary_type = Some("Ubuntu 16.04 LTS".into());
    let d = C4Diagram {
        kind: C4Kind::Deployment,
        title: None,
        elements: vec![node],
        relations: vec![],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // The explicit type text replaces the default "[Deployment Node]" tag.
    assert!(svg.contains("[Ubuntu 16.04 LTS]"));
    assert!(!svg.contains("[Deployment Node]"));
}
