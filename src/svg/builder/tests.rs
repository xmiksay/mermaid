use super::*;

#[test]
fn escapes_specials() {
    assert_eq!(escape("a < b & c"), "a &lt; b &amp; c");
    assert_eq!(escape("\"quoted\""), "&quot;quoted&quot;");
}

#[test]
fn formats_numbers() {
    assert_eq!(fnum(1.0), "1");
    assert_eq!(fnum(1.5), "1.5");
    assert_eq!(fnum(1.123456), "1.123");
    assert_eq!(fnum(-2.0), "-2");
}

#[test]
fn builds_valid_svg_envelope() {
    let mut b = SvgBuilder::new(100.0, 50.0);
    b.rect(0.0, 0.0, 100.0, 50.0, "fill=\"red\"");
    let svg = b.finish();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("viewBox=\"0 0 100 50\""));
    assert!(svg.contains("fill=\"red\""));
    assert!(svg.contains("font-family=\"sans-serif\""));
    assert!(svg.contains("font-size=\"14\""));
}

#[test]
fn envelope_is_responsive() {
    let svg = SvgBuilder::new(120.0, 80.0).finish();
    assert!(svg.contains("width=\"100%\""));
    assert!(svg.contains("style=\"max-width: 120px;\""));
    assert!(svg.contains("viewBox=\"0 0 120 80\""));
    // No fixed pixel height on the root element.
    assert!(!svg.contains("height=\""));
}

#[test]
fn text_decodes_entities_into_content() {
    let mut b = SvgBuilder::new(50.0, 20.0);
    b.text(10.0, 10.0, "", "x #gt; y");
    let svg = b.finish();
    // `#gt;` → `>` → XML-escaped back to `&gt;` (never leaks the raw `#gt;`).
    assert!(svg.contains("x &gt; y"));
    assert!(!svg.contains("#gt;"));
}

#[test]
fn applies_custom_font() {
    let theme = Theme::default_theme()
        .with_font("Inter, sans-serif")
        .with_font_size(16.0);
    let svg = SvgBuilder::new(10.0, 10.0).theme(&theme).finish();
    assert!(svg.contains("font-family=\"Inter, sans-serif\""));
    assert!(svg.contains("font-size=\"16\""));
}

#[test]
fn non_responsive_emits_fixed_size() {
    let mut b = SvgBuilder::new(120.0, 80.0);
    b.responsive = false;
    let svg = b.finish();
    assert!(svg.contains("width=\"120\""));
    assert!(svg.contains("height=\"80\""));
    assert!(!svg.contains("max-width"));
    assert!(!svg.contains("width=\"100%\""));
}

#[test]
fn splits_labels_on_br_and_newlines() {
    assert_eq!(split_label_lines("one line"), vec!["one line"]);
    assert_eq!(split_label_lines("a<br>b"), vec!["a", "b"]);
    assert_eq!(split_label_lines("a<br/>b<br />c"), vec!["a", "b", "c"]);
    assert_eq!(split_label_lines("a<BR/>b"), vec!["a", "b"]);
    assert_eq!(split_label_lines("a<br  / >b"), vec!["a", "b"]);
    // Real newline and the two-char literal escape both split.
    assert_eq!(split_label_lines("a\nb"), vec!["a", "b"]);
    assert_eq!(split_label_lines("a\\nb"), vec!["a", "b"]);
    // Each line is trimmed of surrounding whitespace.
    assert_eq!(split_label_lines("a <br/> b"), vec!["a", "b"]);
}

#[test]
fn br_not_matched_inside_words() {
    assert_eq!(split_label_lines("abrupt"), vec!["abrupt"]);
    assert_eq!(split_label_lines("<break>"), vec!["<break>"]);
}

#[test]
fn multiline_text_emits_tspans_no_literal_br() {
    let mut b = SvgBuilder::new(200.0, 60.0);
    b.text(
        50.0,
        30.0,
        "text-anchor=\"middle\"",
        "line1<br/>line2<br/>line3",
    );
    let svg = b.finish();
    assert_eq!(svg.matches("<tspan").count(), 3);
    assert!(svg.contains(">line1</tspan>"));
    assert!(svg.contains(">line3</tspan>"));
    assert!(!svg.contains("br/"));
    assert!(!svg.contains("&lt;br"));
}

#[test]
fn inline_html_styles_render_as_tspans() {
    let mut b = SvgBuilder::new(200.0, 40.0);
    b.text(
        50.0,
        20.0,
        "text-anchor=\"middle\"",
        "<b>bold</b> <i>it</i> <u>u</u>",
    );
    let svg = b.finish();
    assert!(svg.contains("font-weight=\"bold\">bold</tspan>"));
    assert!(svg.contains("font-style=\"italic\">it</tspan>"));
    assert!(svg.contains("text-decoration=\"underline\">u</tspan>"));
    // The tags themselves never leak as literal text.
    assert!(!svg.contains("&lt;b&gt;"));
}

#[test]
fn inline_style_carries_across_br() {
    // #187: a tag opened before a <br> must still style the next line.
    let mut b = SvgBuilder::new(200.0, 60.0);
    b.text(
        50.0,
        30.0,
        "text-anchor=\"middle\"",
        "<b>line1<br>line2</b>",
    );
    let svg = b.finish();
    // Both stacked lines are bold, not just the first.
    assert_eq!(svg.matches("font-weight=\"bold\"").count(), 2);
    assert!(svg.contains(">line1</tspan>"));
    assert!(svg.contains(">line2</tspan>"));
}

#[test]
fn inline_html_color_span_and_link() {
    let mut b = SvgBuilder::new(200.0, 40.0);
    b.text(
        50.0,
        20.0,
        "",
        "<span style=\"color:red\">r</span><a href=\"http://x\">y</a>",
    );
    let svg = b.finish();
    assert!(svg.contains("fill=\"red\">r</tspan>"));
    assert!(svg.contains("<a href=\"http://x\"><tspan"));
    assert!(svg.contains(">y</tspan></a>"));
}

#[test]
fn unknown_tags_strip_to_plain_text() {
    let mut b = SvgBuilder::new(200.0, 40.0);
    b.text(50.0, 20.0, "", "a<div>b</div>c");
    let svg = b.finish();
    // Stripped, merged, and kept on the single-line fast path (no tspans).
    assert!(svg.contains(">abc</text>"));
    assert!(!svg.contains("<tspan"));
    assert!(!svg.contains("div"));
}

#[test]
fn plain_single_line_stays_bare_text() {
    let mut b = SvgBuilder::new(200.0, 40.0);
    b.text(50.0, 20.0, "", "just text");
    let svg = b.finish();
    assert!(svg.contains(">just text</text>"));
    assert!(!svg.contains("<tspan"));
}

#[test]
fn curve_basis_degenerate() {
    assert_eq!(curve_basis_path(&[]), "");
    assert_eq!(curve_basis_path(&[(3.0, 4.0)]), "M3 4");
}

#[test]
fn curve_basis_two_points_is_straight() {
    let d = curve_basis_path(&[(0.0, 0.0), (10.0, 0.0)]);
    assert_eq!(d, "M0 0L10 0");
    assert!(!d.contains('C'));
}

#[test]
fn curve_basis_three_points_curves_with_exact_endpoints() {
    let pts = [(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)];
    let d = curve_basis_path(&pts);
    assert!(d.starts_with("M0 0"));
    assert!(d.contains('C'));
    // Endpoint exactness: path must end at the last point.
    assert!(d.ends_with("L20 0"));
}

#[test]
fn curve_linear_is_straight_segments() {
    assert_eq!(curve_linear_path(&[]), "");
    assert_eq!(curve_linear_path(&[(3.0, 4.0)]), "M3 4");
    let d = curve_linear_path(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]);
    assert_eq!(d, "M0 0L10 10L20 0");
    assert!(!d.contains('C'));
}

#[test]
fn curve_step_turns_at_midpoints_and_reaches_endpoint() {
    assert_eq!(curve_step_path(&[]), "");
    assert_eq!(curve_step_path(&[(3.0, 4.0)]), "M3 4");
    let d = curve_step_path(&[(0.0, 0.0), (20.0, 10.0)]);
    // Mid-x is 10: go to (10,0), (10,10), then the true endpoint (20,10).
    assert_eq!(d, "M0 0L10 0L10 10L20 10");
    assert!(!d.contains('C'));
}
