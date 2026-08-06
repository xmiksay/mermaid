use super::*;

fn plain(text: &str) -> Span {
    Span {
        text: text.to_string(),
        ..Span::default()
    }
}

#[test]
fn plain_label_is_single_unstyled_span() {
    assert_eq!(parse_spans("hello world"), vec![plain("hello world")]);
    assert!(parse_spans("hello")[0].is_plain());
}

#[test]
fn bold_italic_underline() {
    assert_eq!(
        parse_spans("<b>x</b>"),
        vec![Span {
            text: "x".into(),
            bold: true,
            ..Span::default()
        }]
    );
    assert!(parse_spans("<i>x</i>")[0].italic);
    assert!(parse_spans("<em>x</em>")[0].italic);
    assert!(parse_spans("<strong>x</strong>")[0].bold);
    assert!(parse_spans("<u>x</u>")[0].underline);
}

#[test]
fn mixed_and_nested_runs() {
    let spans = parse_spans("a<b>b<i>c</i></b>d");
    assert_eq!(spans.len(), 4);
    assert_eq!(spans[0], plain("a"));
    assert!(spans[1].bold && !spans[1].italic);
    assert!(spans[2].bold && spans[2].italic);
    assert_eq!(spans[3], plain("d"));
}

#[test]
fn span_color_and_link() {
    let s = parse_spans("<span style=\"color:red\">r</span>");
    assert_eq!(s[0].color.as_deref(), Some("red"));
    let a = parse_spans("<a href=\"https://x\">y</a>");
    assert_eq!(a[0].href.as_deref(), Some("https://x"));
}

#[test]
fn unknown_tags_are_stripped_not_escaped() {
    // `<div>`/`<script>` are unknown → removed, text kept and merged.
    assert_eq!(parse_spans("a<div>b</div>c"), vec![plain("abc")]);
    assert_eq!(parse_spans("<script>x</script>y"), vec![plain("xy")]);
}

#[test]
fn bare_angle_brackets_stay_literal() {
    // A `<` that doesn't open a tag is kept verbatim (decoded later).
    assert_eq!(parse_spans("a < b"), vec![plain("a < b")]);
    assert_eq!(parse_spans("x <"), vec![plain("x <")]);
}

#[test]
fn entities_decode_inside_runs() {
    // `#gt;` must not be seen as a tag; it decodes after tag scanning.
    assert_eq!(parse_spans("a #lt;b#gt; c"), vec![plain("a <b> c")]);
    assert_eq!(parse_spans("<b>#hearts;</b>")[0].text, "♥");
}

#[test]
fn strip_tags_leaves_plain_labels_untouched() {
    assert_eq!(strip_tags("hello #gt; world"), "hello #gt; world");
    assert_eq!(strip_tags("a < b"), "a < b");
    assert_eq!(strip_tags("<b>bold</b> and <i>it</i>"), "bold and it");
}

#[test]
fn markdown_string_emphasis_becomes_styled_spans() {
    let bold = parse_spans("`**bold**`");
    assert_eq!(
        bold,
        vec![Span {
            text: "bold".into(),
            bold: true,
            ..Span::default()
        }]
    );
    let italic = parse_spans("`*it*`");
    assert!(italic[0].italic && !italic[0].bold);
    // Underscore forms toggle the same way.
    assert!(parse_spans("`__b__`")[0].bold);
    assert!(parse_spans("`_i_`")[0].italic);
}

#[test]
fn markdown_string_mixes_plain_and_emphasis() {
    let spans = parse_spans("`a **b** c`");
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0], plain("a "));
    assert!(spans[1].bold && spans[1].text == "b");
    assert_eq!(spans[2], plain(" c"));
}

#[test]
fn markdown_string_nests_bold_and_italic() {
    let spans = parse_spans("`**b _bi_**`");
    assert!(spans[0].bold && !spans[0].italic);
    let bi = spans.iter().find(|s| s.text == "bi").unwrap();
    assert!(bi.bold && bi.italic);
}

#[test]
fn plain_fenced_string_is_single_span() {
    // No emphasis markers → one plain run, so the bare-<text> fast path holds.
    assert_eq!(parse_spans("`plain`"), vec![plain("plain")]);
}

#[test]
fn tag_stack_carries_across_lines() {
    // #187: <b> opened on the first line still styles the second line.
    let lines = ["<b>line1", "line2</b>", "plain"];
    let parsed = parse_lines(&lines);
    assert!(parsed[0][0].bold);
    assert!(parsed[1][0].bold);
    assert!(!parsed[2][0].bold);
}

#[test]
fn quoted_gt_in_attr_does_not_close_early() {
    let s = parse_spans("<span style=\"color:red\" title=\"a>b\">z</span>");
    assert_eq!(s[0].text, "z");
    assert_eq!(s[0].color.as_deref(), Some("red"));
}
