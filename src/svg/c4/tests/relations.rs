use super::*;
use crate::parse::{C4Kind, C4RelDirection, C4Relation};

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
fn rel_is_straight_and_unbacked() {
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
    // Straight line (upstream #327), #444444, width 1 — no Bézier control point.
    assert!(svg.contains(" L"));
    assert!(!svg.contains(" Q"));
    assert!(svg.contains("stroke=\"#444444\" stroke-width=\"1\""));
    // No translucent label background rect.
    assert!(!svg.contains("fill-opacity=\"0.5\""));
    // techn rendered italic as [HTTPS].
    assert!(svg.contains(">[HTTPS]<"));
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
