use super::layout::context_sample;
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
            reversed: false,
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
            reversed: false,
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
            reversed: false,
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
            reversed: false,
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
            reversed: false,
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
fn label_does_not_land_on_an_unrelated_node() {
    // Regression for #366, adapted for the #370 width-based row wrap: with
    // `samples/c4.mmd`'s five elements, Internet Banking sits directly below
    // Banking Customer and Mainframe sits two rows below it in the same
    // column (row-flow, #370), so a straight connector from Banking Customer
    // to Mainframe — and its label — pass directly over Internet Banking.
    // The label must dodge it instead of rendering on its fill.
    let elements = context_sample().elements;
    let p = grid_of(elements.clone(), None);
    let obstacle = rect_of(&p, "SystemAA");
    let source = rect_of(&p, "customerA");
    let target = rect_of(&p, "SystemC");
    // Sanity: the obstacle must actually sit between the two endpoints
    // (same column, middle row) or this test would pass vacuously.
    assert!(
        obstacle.1 > source.1 && obstacle.1 < target.1,
        "Internet Banking {obstacle:?} must sit between Banking Customer {source:?} and Mainframe {target:?}"
    );
    assert!(
        obstacle.0 < source.0 + source.2 && source.0 < obstacle.0 + obstacle.2,
        "Internet Banking {obstacle:?} must share a column with Banking Customer {source:?}"
    );

    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements,
        relations: vec![C4Relation {
            from: "customerA".into(),
            to: "SystemC".into(),
            label: "Uses".into(),
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
            reversed: false,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());

    let rect_start = svg.find("fill=\"#e8e8e8\"").expect("label background rect");
    let tag_start = svg[..rect_start].rfind("<rect ").expect("rect tag start");
    let attr = |name: &str| -> f64 {
        let key = format!("{name}=\"");
        let start = svg[tag_start..].find(&key).unwrap() + tag_start + key.len();
        let end = svg[start..].find('"').unwrap() + start;
        svg[start..end].parse().unwrap()
    };
    let label_rect = (attr("x"), attr("y"), attr("width"), attr("height"));

    let overlap_x =
        (label_rect.0 + label_rect.2).min(obstacle.0 + obstacle.2) - label_rect.0.max(obstacle.0);
    let overlap_y =
        (label_rect.1 + label_rect.3).min(obstacle.1 + obstacle.3) - label_rect.1.max(obstacle.1);
    assert!(
        overlap_x <= 0.0 || overlap_y <= 0.0,
        "label {label_rect:?} must not overlap Internet Banking's box {obstacle:?}"
    );
}

#[test]
fn reversed_pair_labels_do_not_stack() {
    // #381: RelIndex(2, web, api, ...) and RelIndex(4, api, web, ...) are
    // the exact same line, reversed. Their labels must not land on top of
    // one another once both are placed.
    let rel = |from: &str, to: &str, label: &str| C4Relation {
        from: from.into(),
        to: to.into(),
        label: label.into(),
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        direction: C4RelDirection::Default,
        bidirectional: false,
        reversed: false,
    };
    let d = C4Diagram {
        kind: C4Kind::Dynamic,
        elements: vec![person("web", "Web"), person("api", "API")],
        relations: vec![
            rel("web", "api", "POST /checkout"),
            rel("api", "web", "Order confirmation"),
        ],
        ..Default::default()
    };
    let s = layout_scene(&d, &HashMap::new(), &Theme::default());
    let rects: Vec<Rect> = s.rels.iter().filter_map(RelPlan::label_rect).collect();
    assert_eq!(rects.len(), 2);
    assert_eq!(
        crate::svg::label_layout::rect_overlap_area(rects[0], rects[1]),
        0.0,
        "label backgrounds {rects:?} overlap"
    );
}

#[test]
fn arrow_tip_sits_on_the_path_end() {
    // The marker's tip is x=10 of its 10-unit viewBox; refX must match so the
    // tip lands exactly on the clipped border point.
    let d = C4Diagram {
        elements: vec![person("a", "A")],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("refX=\"10\""), "{svg}");
    assert!(svg.contains("M0,0 L10,5 L0,10 z"));
}
