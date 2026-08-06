use crate::parse::parse;
use crate::svg::sequence::render;
use crate::svg::theme::Theme;

fn build(s: &str) -> crate::parse::SequenceDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Sequence(d) => d,
        _ => panic!("not sequence"),
    }
}

const SAMPLE: &str = "zenuml\n\
    title Login flow\n\
    @Actor User\n@Boundary Page\n@Control Auth\n@Database DB\n\
    User -> Page.login(name, pass) {\n\
        Auth.verify(name, pass) {\n\
            found = DB.query(name)\n\
            if (found) {\n\
                return token\n\
            } else {\n\
                return denied\n\
            }\n\
        }\n\
        Page.render()\n\
    }\n";

#[test]
fn draws_activation_bars() {
    let svg = render(&build(SAMPLE), &Theme::default());
    // Activation bands are fixed-width (10px) rects on the receivers'
    // lifelines: one per synchronous call (login/verify/query/render).
    let bands = svg.matches("width=\"10\"").count();
    assert_eq!(bands, 4, "one activation band per call");
}

#[test]
fn alt_uses_zenuml_fragment_chrome() {
    let svg = render(&build(SAMPLE), &Theme::default());
    // ◇ Alt header row with the guard `[ found ]` beneath it (issue #315).
    assert!(svg.contains("\u{25c7} Alt"), "diamond operator header");
    assert!(svg.contains(">[ found ]<"), "guard row beneath header");
    assert!(svg.contains(">[ else ]<"), "else compartment guard");
    // The classic-sequence gray bar and the shaded else region are gone.
    assert!(!svg.contains(">alt [found]<"), "no gray operator bar");
    assert!(!svg.contains("rgba(0,0,0,0.04)"), "else region not shaded");
}

#[test]
fn emits_hierarchical_numbers() {
    let svg = render(&build(SAMPLE), &Theme::default());
    // Forward calls and their labels sit in separate runs: the number is a
    // gray `<span>`, the text keeps the default fill.
    assert!(svg.contains(">1</tspan>"), "top call is 1");
    assert!(svg.contains("> login(name, pass)</tspan>"));
    assert!(svg.contains(">1.1</tspan>") && svg.contains("> verify(name, pass)</tspan>"));
    assert!(svg.contains(">1.1.1</tspan>") && svg.contains("> query(name)</tspan>"));
    assert!(svg.contains(">1.2</tspan>") && svg.contains("> render()</tspan>"));
}

#[test]
fn returns_are_numbered_and_dimmed() {
    let svg = render(&build(SAMPLE), &Theme::default());
    // Dashed returns now carry numbers (issue #315): the call's own reply
    // nests under it, explicit returns number at their branch level.
    assert!(svg.contains(">1.1.1.1</tspan>") && svg.contains("> found</tspan>"));
    assert!(svg.contains(">1.1.2.1</tspan>") && svg.contains("> token</tspan>"));
    assert!(svg.contains(">1.1.2.2</tspan>") && svg.contains("> denied</tspan>"));
    // The number run is dimmed gray (fg_muted), not the label color.
    assert!(svg.contains(&format!(
        "fill=\"{}\">1.1.1.1</tspan>",
        Theme::default().fg_muted
    )));
}

#[test]
fn first_message_has_an_arrowhead() {
    // ZenUML `->` is a sync call and carries a filled head like the rest, so
    // `login` no longer ends flush at the activation bar (issue #315). The
    // explicit-arrow form now classifies as `SolidArrow`, not bare `Solid`.
    let d = build(SAMPLE);
    let first = d
        .items
        .iter()
        .find_map(|i| match i {
            crate::parse::SequenceItem::Message(m) => Some(m),
            _ => None,
        })
        .unwrap();
    assert_eq!(first.arrow, crate::parse::ArrowKind::SolidArrow);
}

#[test]
fn participants_are_white_boxed_with_left_icon() {
    let svg = render(&build(SAMPLE), &Theme::default());
    // Every participant box is white-filled with a gray border — not the
    // lavender actor box (issue #315). Four participants → four such rects.
    assert_eq!(
        svg.matches("fill=\"#fff\" stroke=\"#666\" stroke-width=\"1\" rx=\"2\"")
            .count(),
        4,
        "white/gray participant boxes"
    );
}

#[test]
fn participants_are_top_only() {
    let svg = render(&build(SAMPLE), &Theme::default());
    // Each participant name appears exactly once — no bottom actor row.
    assert_eq!(svg.matches(">User<").count(), 1, "no repeated bottom row");
    assert_eq!(svg.matches(">DB<").count(), 1);
}

#[test]
fn title_goes_in_a_frame_header() {
    let svg = render(&build(SAMPLE), &Theme::default());
    assert!(svg.contains(">Login flow<"), "title rendered");
    // Not the centered sequence title (which would be bold 18px).
    assert!(!svg.contains("font-size=\"18\""));
}

#[test]
fn boundary_bar_fits_inside_the_box() {
    // Page is @Boundary: its glyph is a vertical bar joined to a circle by
    // a short stem. The bar previously sat 3px left of the box's own
    // border (bar_x = box_x - 3), poking outside it. Verify the bar's x
    // never falls left of the box it's drawn in.
    let svg = render(&build(SAMPLE), &Theme::default());
    let box_marker = "fill=\"#fff\" stroke=\"#666\" stroke-width=\"1\" rx=\"2\"";
    let rect_start = svg
        .match_indices(box_marker)
        .nth(1)
        .map(|(i, _)| i)
        .expect("second participant box (Page)");
    let rect_tag_start = svg[..rect_start].rfind("<rect x=\"").unwrap();
    let box_x: f64 = svg[rect_tag_start + "<rect x=\"".len()..]
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let line_marker = "<line x1=\"";
    let line_start = svg[rect_start..].find(line_marker).unwrap() + rect_start;
    let bar_x: f64 = svg[line_start + line_marker.len()..]
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        bar_x >= box_x,
        "boundary glyph bar (x={bar_x}) pokes out past the box's left edge (x={box_x})"
    );
}

#[test]
fn synthesized_returns_are_suppressed() {
    // A braced call with no assignment implies its return via the activation
    // end; no empty dashed reply arrow is drawn for it.
    let svg = render(
        &build("zenuml\nA.run() {\n  B.step()\n}\n"),
        &Theme::default(),
    );
    // The only dashed arrows present would be labeled returns; none here.
    assert!(!svg.contains("stroke-dasharray=\"6 4\""));
}
