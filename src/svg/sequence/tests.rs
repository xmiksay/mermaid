use super::*;
use crate::parse::parse;

fn build(s: &str) -> SequenceDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Sequence(d) => d,
        _ => panic!("not sequence"),
    }
}

#[test]
fn basic_envelope() {
    let svg = render(
        &build("sequenceDiagram\ntitle Login\nalice->>bob: hi\n"),
        &Theme::default(),
    );
    assert!(svg.contains(">Login<"));
    assert!(svg.contains(">hi<"));
    assert!(svg.contains("arrow-filled"));
    // Title uses upstream regular weight, not bold (#332).
    assert!(svg.contains("font-size=\"18\">Login</text>"));
    assert!(!svg.contains("font-weight=\"bold\">Login<"));
}

#[test]
fn plain_line_arrows_have_no_marker() {
    // `->` / `-->` are bare lines with no arrowhead (issue #57).
    let svg = render(&build("sequenceDiagram\nA->B: x\n"), &Theme::default());
    assert!(!svg.contains("marker-end"));
    assert!(!svg.contains("marker-start"));
    let svg = render(&build("sequenceDiagram\nA-->B: y\n"), &Theme::default());
    assert!(!svg.contains("marker-end"));
    assert!(!svg.contains("marker-start"));
}

#[test]
fn bidirectional_arrow_marks_both_ends() {
    let svg = render(
        &build("sequenceDiagram\nAlice<<->>Bob: hi\n"),
        &Theme::default(),
    );
    assert!(svg.contains("marker-start=\"url(#arrow-filled)\""));
    assert!(svg.contains("marker-end=\"url(#arrow-filled)\""));
}

#[test]
fn created_participant_has_no_top_row_box() {
    // Carl's box is drawn inline at the create point, so its lifeline does
    // not span the full height and there is no top-row box for it. We at
    // least verify the diagram renders and both Bob (normal) and Carl draw.
    let svg = render(
        &build(
            "sequenceDiagram\nAlice->>Bob: Hello\ncreate participant Carl\nAlice->>Carl: Hi Carl\n",
        ),
        &Theme::default(),
    );
    assert!(svg.contains(">Carl<"), "created participant box is drawn");
    assert!(svg.contains(">Hi Carl<"), "creating message is drawn");
}

/// `(x, width)` of every `<rect>` in `svg`.
fn rect_extents(svg: &str) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find("<rect ") {
        let tag_start = start + "<rect ".len();
        let tag_end = rest[tag_start..]
            .find('/')
            .map_or(rest.len(), |i| tag_start + i);
        let tag = &rest[tag_start..tag_end];
        let grab = |attr: &str| -> f64 {
            let s = tag.find(attr).unwrap() + attr.len();
            let e = s + tag[s..].find('"').unwrap();
            tag[s..e].parse::<f64>().unwrap()
        };
        out.push((grab("x=\""), grab("width=\"")));
        rest = &rest[tag_end..];
    }
    out
}

#[test]
fn note_left_of_lone_participant_stays_within_canvas() {
    // A single-participant diagram's box sits close to the canvas edge; a
    // fixed-width `note left of`/`note right of` box anchored off that
    // lifeline extends well past it. The canvas used to be sized from
    // participant positions only, so both notes rendered cropped (bug).
    let svg = render(
        &build(
            "sequenceDiagram\nparticipant A\n\
             note left of A: pinned to the left side of the lone participant\n\
             note right of A: pinned to the right side of the lone participant\n",
        ),
        &Theme::default(),
    );
    let viewbox_w = svg
        .split("viewBox=\"0 0 ")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .expect("viewBox present")
        .parse::<f64>()
        .expect("viewBox width is numeric");
    for (x, w) in rect_extents(&svg) {
        assert!(x >= -0.01, "rect x={x} is cropped off the left canvas edge");
        assert!(
            x + w <= viewbox_w + 0.01,
            "rect right edge {} exceeds canvas width {viewbox_w}",
            x + w
        );
    }
}

/// x of the opening `<text ...>` tag whose content is `needle` (including
/// the surrounding `>`/`<` so it doesn't match a text node's own middle).
fn text_x(svg: &str, needle: &str) -> f64 {
    let end = svg
        .find(needle)
        .unwrap_or_else(|| panic!("missing {needle}"));
    let open = svg[..end].rfind("<text ").expect("preceding <text tag");
    let tag = &svg[open..end];
    let s = tag.find("x=\"").expect("x attr") + 3;
    let e = s + tag[s..].find('"').unwrap();
    tag[s..e].parse::<f64>().unwrap()
}

#[test]
fn narrow_opt_block_condition_label_clears_the_tab() {
    // A single-participant `opt` frame (e.g. wrapping a self-message) is
    // barely wider than its own pentagon tab; the guard label used to
    // center in the frame regardless of the tab's width, landing on top
    // of it instead of after it (bug).
    let svg = render(
        &build(
            "sequenceDiagram\nparticipant Server\nopt caching enabled\nServer->>Server: Store\nend\n",
        ),
        &Theme::default(),
    );
    let tab_x = text_x(&svg, ">opt<");
    let label_x = text_x(&svg, ">[caching enabled]<");
    assert!(
        label_x + 0.01 >= tab_x + BLOCK_LABEL_W,
        "condition label (x={label_x}) overlaps the tab (x={tab_x}, width={BLOCK_LABEL_W})"
    );
}
