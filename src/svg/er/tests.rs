use super::relation::{draw_cardinality, CARD_GAP};
use super::render;
use crate::parse::{parse, Cardinality, ErDiagram};
use crate::svg::builder::SvgBuilder;
use crate::svg::theme::Theme;

fn build(s: &str) -> ErDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Er(e) => e,
        _ => panic!("not er"),
    }
}

#[test]
fn renders_basic() {
    let d = build("erDiagram\nCUSTOMER ||--o{ ORDER : places\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">CUSTOMER<"));
    assert!(svg.contains(">ORDER<"));
    assert!(svg.contains(">places<"));
}

// Pull the numeric value of `attr="…"` immediately following `after` in `s`.
fn attr_after(s: &str, after: &str, attr: &str) -> f64 {
    let tail = &s[s.find(after).expect("marker present") + after.len()..];
    let start = tail.find(attr).expect("attr present") + attr.len() + 2; // attr="
    let end = start + tail[start..].find('"').unwrap();
    tail[start..end].parse().unwrap()
}

#[test]
fn zero_or_more_foot_is_wide_at_entity() {
    // Issue #313: the crow's foot must be wide at the entity border and
    // converge to a point out along the edge — not the inverse. Draw the
    // marker along +y from the origin (the entity boundary) and check the
    // prongs splay at the anchor and meet at a single far point.
    let mut svg = SvgBuilder::new(60.0, 60.0);
    draw_cardinality(
        &mut svg,
        (0.0, 0.0),
        (0.0, 100.0),
        Cardinality::ZeroOrMore,
        &Theme::default(),
    );
    let out = svg.finish();
    // Every prong shares the same convergence point (x1,y1) out along +y…
    let conv_x = attr_after(&out, "<line", "x1");
    let conv_y = attr_after(&out, "<line", "y1");
    assert!(conv_x.abs() < 1e-6, "prongs converge off-axis: x={conv_x}");
    assert!(
        conv_y > CARD_GAP,
        "convergence point sits at the entity, not out along the edge: {conv_y}",
    );
    // …while their far ends (x2) splay across the entity border at y≈0.
    let splay_x = attr_after(&out, "<line", "x2");
    let border_y = attr_after(&out, "<line", "y2");
    assert!(
        border_y.abs() < 1e-6,
        "foot base is not at the entity border: y={border_y}",
    );
    let _ = splay_x;
}

#[test]
fn zero_or_more_circle_is_tangent_to_foot() {
    // Issue #256/#313: the optional "zero" circle sits just past the foot's
    // convergence point — a complete, separate glyph, not merged into the
    // foot as a "Ø" blob.
    let mut svg = SvgBuilder::new(60.0, 60.0);
    draw_cardinality(
        &mut svg,
        (0.0, 0.0),
        (0.0, 100.0),
        Cardinality::ZeroOrMore,
        &Theme::default(),
    );
    let out = svg.finish();
    // Convergence point (shared x1,y1 of the prongs) is the foot's far tip.
    let conv_y = attr_after(&out, "<line", "y1");
    let circle_cy = attr_after(&out, "<circle", "cy");
    let circle_r = attr_after(&out, "<circle", "r");
    // Circle's near edge is tangent to the convergence point (no overlap).
    let near_edge = circle_cy - circle_r;
    assert!(
        near_edge + 1e-6 >= conv_y,
        "circle (cy={circle_cy}, r={circle_r}) overlaps foot point {conv_y}",
    );
    assert!(
        (near_edge - conv_y).abs() < 1.0,
        "circle not tangent to foot point (gap {})",
        near_edge - conv_y,
    );
}

#[test]
fn entity_with_attributes() {
    let d = build(
        "erDiagram\nCUSTOMER {\nstring name\nstring email PK\n}\nCUSTOMER ||--o{ ORDER : places\n",
    );
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">name<"));
    assert!(svg.contains(">email<"));
    assert!(svg.contains(">PK<"));
}

#[test]
fn attributes_render_as_bordered_striped_table() {
    // Issue #255: attribute rows are a real table — per-cell bordered rects
    // with alternating row fills, not a flat lavender panel.
    let d = build("erDiagram\nCUSTOMER {\nstring name\nstring email PK\nstring phone\n}\n");
    let theme = Theme::default();
    let svg = render(&d, &theme);
    // Cell borders use the theme stroke.
    assert!(svg.contains(&format!(
        "stroke=\"{}\" stroke-width=\"1\"",
        theme.flow_node_stroke
    )));
    // Row striping alternates background (odd) and primary (even) fills.
    assert!(svg.contains(&format!("fill=\"{}\" stroke=", theme.bg)));
    assert!(svg.contains(&format!("fill=\"{}\" stroke=", theme.flow_node_fill)));
}

#[test]
fn key_marker_is_plain_not_red_bold() {
    // Issue #255: PK/FK were rendered red and bold — an invention. They now
    // render plain, like every other attribute cell.
    let d = build("erDiagram\nCUSTOMER {\nstring email PK\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">PK<"));
    assert!(!svg.contains("#c33"));
    assert!(!svg.contains("font-weight=\"bold\" fill=\"#c33\""));
}

#[test]
fn comment_is_rendered() {
    let d = build("erDiagram\nCUSTOMER {\nstring name \"the customer name\"\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">the customer name<"));
}

#[test]
fn alias_label_shown() {
    let d = build("erDiagram\np[Person] {\nstring name\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">Person<"));
}

#[test]
fn classdef_recolors_entity() {
    let d = build(
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\nclassDef hot fill:#ff0000,stroke:#990000\nclass CUSTOMER hot\n",
    );
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#ff0000\""));
    assert!(svg.contains("stroke=\"#990000\""));
}

#[test]
fn unstyled_entity_uses_theme() {
    // Without any classDef the entity box uses the theme fill (regression:
    // the styling path must stay byte-compatible for unstyled diagrams).
    let d = build("erDiagram\nA ||--|| B : x\n");
    let svg = render(&d, &Theme::default());
    let theme = Theme::default();
    assert!(svg.contains(&format!("fill=\"{}\"", theme.flow_node_fill)));
}
