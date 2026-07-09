use super::*;
use crate::parse::parse;

fn build(s: &str) -> ClassDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Class(c) => c,
        _ => panic!("not class"),
    }
}

#[test]
fn style_applies_to_class_box() {
    let d = build("classDiagram\nAnimal --> Dog\nstyle Animal fill:#abc\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#abc\""));
}

#[test]
fn class_label_renders_without_brackets() {
    let d = build("classDiagram\nclass Animal[\"Animal with a label\"]\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("Animal with a label"));
    // No brackets and no duplicate bare-name box.
    assert!(!svg.contains('['));
    assert!(!svg.contains(']'));
}

#[test]
fn note_renders_yellow_box() {
    let d = build("classDiagram\nclass Duck\nnote for Duck \"can fly\"\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("can fly"));
    assert!(svg.contains("#fff5ad"));
    // Dashed connector to the target class.
    assert!(svg.contains("stroke-dasharray=\"4 3\""));
}

#[test]
fn click_wraps_class_in_anchor() {
    let d = build("classDiagram\nclass Shape\nclick Shape href \"https://example.com\"\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("<a href=\"https://example.com\">"));
}

#[test]
fn nested_namespace_draws_both_frames_with_labels() {
    let d = build(
        "classDiagram\n\
         namespace Outer[\"Outer Label\"] {\n\
         class A\n\
         namespace Inner {\n\
         class B\n\
         }\n\
         }\n",
    );
    let svg = render(&d, &Theme::default());
    // Both frames render; the outer shows its bracket label, not raw text.
    assert!(svg.contains("Outer Label"));
    assert!(svg.contains(">Inner<"));
    assert!(!svg.contains('['));
    // Two solid light-yellow namespace frames are drawn.
    assert!(svg.matches("fill=\"#ffffde\"").count() >= 2);
}

/// Every `<rect .../>` as `(x, y, w, h, attrs)`.
fn rects(svg: &str) -> Vec<(f64, f64, f64, f64, String)> {
    svg.match_indices("<rect ")
        .map(|(i, _)| {
            let tag = &svg[i..i + svg[i..].find("/>").unwrap()];
            let get = |k: &str| -> f64 {
                let start = tag.find(&format!("{k}=\"")).unwrap() + k.len() + 2;
                let end = tag[start..].find('"').unwrap();
                tag[start..start + end].parse().unwrap()
            };
            (
                get("x"),
                get("y"),
                get("width"),
                get("height"),
                tag.to_string(),
            )
        })
        .collect()
}

#[test]
fn outsider_class_does_not_straddle_namespace_border() {
    // `Cat` is declared outside `namespace Domain`; its box must land fully
    // clear of the dashed frame, never straddling the border (issue #249).
    let d = build(
        "classDiagram\n\
         namespace Domain {\n\
         class Animal\n\
         class Dog\n\
         }\n\
         class Cat\n\
         Animal <|-- Dog\n\
         Animal <|-- Cat\n",
    );
    let svg = render(&d, &Theme::default());
    let all = rects(&svg);
    let frame = all
        .iter()
        .find(|(.., attrs)| attrs.contains("fill=\"#ffffde\""))
        .expect("namespace frame drawn");
    let (fx, fy, fw, fh, _) = frame;
    let (fx1, fy1) = (fx + fw, fy + fh);

    for (x, y, w, h, attrs) in all.iter().filter(|(.., a)| a.contains("rx=\"2\"")) {
        let (x1, y1) = (x + w, y + h);
        let overlaps = *x < fx1 && x1 > *fx && *y < fy1 && y1 > *fy;
        let inside = *x >= *fx && x1 <= fx1 && *y >= *fy && y1 <= fy1;
        // A class box either sits fully inside the frame (member) or fully
        // clear of it (outsider) — it must never straddle the border.
        assert!(
            !overlaps || inside,
            "class box ({x},{y},{w},{h}) straddles namespace frame \
             ({fx},{fy},{fw},{fh}); attrs={attrs}"
        );
    }
}

#[test]
fn memberless_class_draws_three_compartments() {
    // A class with no members still gets the two dividers (three
    // compartments), not a single plain rect (issue #328).
    let d = build("classDiagram\nclass Collar\n");
    let svg = render(&d, &Theme::default());
    // No relations, so every `<line` is a compartment divider: two of them.
    assert_eq!(svg.matches("<line").count(), 2);
}

#[test]
fn namespace_frame_is_solid_with_centered_title() {
    // Upstream draws the namespace as a solid light-yellow rect with the
    // title centered at the top — not a dashed, unfilled, italic-corner box.
    let d = build("classDiagram\nnamespace Domain {\nclass A\nclass B\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#ffffde\""));
    assert!(!svg.contains("stroke=\"#888\""));
    // Title centered, upright (not italic).
    assert!(svg.contains(">Domain<"));
    let title_idx = svg.find(">Domain<").unwrap();
    let tag_start = svg[..title_idx].rfind("<text").unwrap();
    let title_tag = &svg[tag_start..title_idx];
    assert!(title_tag.contains("text-anchor=\"middle\""));
    assert!(!title_tag.contains("font-style=\"italic\""));
}

#[test]
fn stereotype_renders_upright() {
    // `«abstract»` is regular weight, not italic (issue #328).
    let d = build("classDiagram\nclass Animal {\n<<abstract>>\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("«abstract»"));
    assert!(!svg.contains("font-style=\"italic\""));
}

#[test]
fn cssclass_applies_classdef() {
    let d =
        build("classDiagram\nAnimal --> Dog\nclassDef foo fill:#abc\ncssClass \"Animal\" foo\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#abc\""));
}
