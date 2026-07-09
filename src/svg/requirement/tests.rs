use super::*;
use crate::parse::{ReqElement, ReqRelation, Requirement};

#[test]
fn produces_svg() {
    let d = RequirementDiagram {
        requirements: vec![Requirement {
            kind: RequirementKind::Requirement,
            name: "req1".into(),
            id: Some("1".into()),
            text: Some("the req".into()),
            risk: None,
            verifymethod: None,
        }],
        elements: vec![ReqElement {
            name: "e1".into(),
            type_: Some("sim".into()),
            docref: None,
        }],
        relations: vec![ReqRelation {
            from: "e1".into(),
            to: "req1".into(),
            kind: ReqRelationKind::Satisfies,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">req1<"));
    assert!(svg.contains(">e1<"));
    assert!(svg.contains("req-arrow"));
}

#[test]
fn edges_are_thin_dark_gray_with_open_arrowheads() {
    let svg = render_single_relation(ReqRelationKind::Satisfies);
    let edge = &Theme::default().flow_edge_stroke;
    // The relation edge path is dark-gray, not the purple node stroke.
    assert!(svg.contains(&format!(
        "fill=\"none\" stroke=\"{edge}\" stroke-width=\"1.5\""
    )));
    assert!(!svg.contains("fill=\"none\" stroke=\"#9370DB\""));
    // The arrowhead marker is an open (line) chevron: stroked, no fill.
    assert!(svg.contains("id=\"req-arrow\""));
    assert!(svg.contains(&format!(
        "<path d=\"M1,1 L9,5 L1,9\" fill=\"none\" stroke=\"{edge}\""
    )));
}

#[test]
fn relation_label_has_light_gray_background_box() {
    let svg = render_single_relation(ReqRelationKind::Traces);
    assert!(svg.contains("fill=\"#e8e8e8\" fill-opacity=\"0.85\""));
}

#[test]
fn header_stereotype_is_upright_not_italic() {
    let svg = render_single_relation(ReqRelationKind::Satisfies);
    assert!(!svg.contains("font-style=\"italic\""));
}

#[test]
fn header_and_body_match_upstream_format() {
    let d = RequirementDiagram {
        requirements: vec![Requirement {
            kind: RequirementKind::Functional,
            name: "func_req".into(),
            id: Some("2".into()),
            text: Some("must do thing".into()),
            risk: Some("high".into()),
            verifymethod: Some("test".into()),
        }],
        elements: vec![ReqElement {
            name: "user_doc".into(),
            type_: Some("document".into()),
            docref: Some("user-guide.md".into()),
        }],
        relations: vec![ReqRelation {
            from: "user_doc".into(),
            to: "func_req".into(),
            kind: ReqRelationKind::Satisfies,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Title-cased `<<Type>>` headers, not lowercase guillemets.
    assert!(svg.contains(">&lt;&lt;Functional Requirement&gt;&gt;<"));
    assert!(svg.contains(">&lt;&lt;Element&gt;&gt;<"));
    assert!(!svg.contains('«'));
    // Prose `Label: value` body with title-cased enum values.
    assert!(svg.contains(">ID: 2<"));
    assert!(svg.contains(">Text: must do thing<"));
    assert!(svg.contains(">Risk: High<"));
    assert!(svg.contains(">Verification: Test<"));
    assert!(svg.contains(">Type: document<"));
    assert!(svg.contains(">Doc Ref: user-guide.md<"));
    // Relation label uses `<<…>>`, not a guillemet pill.
    assert!(svg.contains(">&lt;&lt;satisfies&gt;&gt;<"));
}

fn render_single_relation(kind: ReqRelationKind) -> String {
    let d = RequirementDiagram {
        requirements: vec![
            Requirement {
                kind: RequirementKind::Requirement,
                name: "a".into(),
                id: None,
                text: None,
                risk: None,
                verifymethod: None,
            },
            Requirement {
                kind: RequirementKind::Requirement,
                name: "b".into(),
                id: None,
                text: None,
                risk: None,
                verifymethod: None,
            },
        ],
        elements: vec![],
        relations: vec![ReqRelation {
            from: "a".into(),
            to: "b".into(),
            kind,
        }],
        ..Default::default()
    };
    render(&d, &Theme::default())
}

#[test]
fn relation_stroke_style_table_matches_upstream() {
    use ReqRelationKind::*;
    // Upstream 11.x: only `contains` is solid; every other relation is
    // dashed with the thin arrowhead.
    for kind in [Copies, Derives, Satisfies, Verifies, Refines, Traces] {
        let svg = render_single_relation(kind);
        assert!(
            svg.contains("stroke-dasharray=\"5 3\""),
            "{kind:?} should be dashed"
        );
        assert!(
            svg.contains("marker-end=\"url(#req-arrow)\""),
            "{kind:?} should use the thin arrowhead"
        );
    }
    let svg = render_single_relation(Contains);
    assert!(
        !svg.contains("stroke-dasharray"),
        "contains should be solid"
    );
    // `contains` puts the crossed circle at the container (`from`) end,
    // so it renders as a `marker-start`, not a `marker-end`.
    assert!(svg.contains("marker-start=\"url(#req-contains)\""));
}

fn two_reqs(a: &str, b: &str) -> Vec<Requirement> {
    [a, b]
        .into_iter()
        .map(|name| Requirement {
            kind: RequirementKind::Requirement,
            name: name.into(),
            id: None,
            text: None,
            risk: None,
            verifymethod: None,
        })
        .collect()
}

// The container is always `from`; the crossed circle must sit at that end,
// i.e. as a `marker-start`, so it renders on the container's box edge.
// `from` is the container regardless of which syntactic direction the
// parser recorded, so both orderings produce the same `marker-start`.
#[test]
fn contains_glyph_sits_at_container_end() {
    // `container - contains -> contained` (forward form).
    let d = RequirementDiagram {
        requirements: two_reqs("container", "contained"),
        elements: vec![],
        relations: vec![ReqRelation {
            from: "container".into(),
            to: "contained".into(),
            kind: ReqRelationKind::Contains,
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("id=\"req-contains\""));
    assert!(svg.contains("marker-start=\"url(#req-contains)\""));
    assert!(!svg.contains("marker-end=\"url(#req-contains)\""));
}

#[test]
fn contains_glyph_at_container_end_for_reverse_form() {
    // End-to-end: `contained <- contains - container` (reverse syntax,
    // matching the issue's `func_req <- contains - test_entity`) parses to
    // container→contained, so the glyph is still a `marker-start`.
    let src = "requirementDiagram\n\
               requirement contained {\n    id: 1\n}\n\
               element container {\n    type: sim\n}\n\
               contained <- contains - container\n";
    let crate::parse::Diagram::Requirement(d) = crate::parse::parse(src).unwrap() else {
        panic!("expected a requirement diagram");
    };
    assert_eq!(d.relations[0].from, "container");
    assert_eq!(d.relations[0].to, "contained");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("marker-start=\"url(#req-contains)\""));
    assert!(!svg.contains("marker-end=\"url(#req-contains)\""));
}
