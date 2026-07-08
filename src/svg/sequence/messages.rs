//! Message arrows, notes, and the arrowhead marker definitions.

use std::collections::HashMap;

use crate::parse::{ArrowKind, NotePosition};

use super::*;

pub(super) fn draw_message(
    svg: &mut SvgBuilder,
    x1: f64,
    x2: f64,
    y: f64,
    arrow: ArrowKind,
    text: &str,
    theme: &Theme,
) {
    let fg = theme.signal_text();
    let arrow_stroke = &theme.arrow_stroke;
    let (dash, start_marker, end_marker) = stroke_for(arrow);
    let dash_attr = if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{dash}\"")
    };
    let marker_attr = |m: Option<&str>, kind: &str| match m {
        Some(name) => format!(" marker-{kind}=\"url(#{name})\""),
        None => String::new(),
    };
    let markers = format!(
        "{}{}",
        marker_attr(start_marker, "start"),
        marker_attr(end_marker, "end")
    );

    if (x1 - x2).abs() < 1e-6 {
        let r = 20.0;
        let d_attr = format!(
            "M{x1} {y}L{xr} {y}L{xr} {y2}L{x1} {y2}",
            x1 = svg_n(x1),
            y = svg_n(y),
            xr = svg_n(x1 + r),
            y2 = svg_n(y + r),
        );
        svg.path(
            &d_attr,
            &format!(
                "fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"{dash_attr}{markers}"
            ),
        );
        if !text.is_empty() {
            svg.text(
                x1 + r + 4.0,
                y + r / 2.0 + 4.0,
                &format!("fill=\"{fg}\" font-size=\"12\""),
                text,
            );
        }
        return;
    }

    svg.line(
        x1,
        y,
        x2,
        y,
        &format!("stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"{dash_attr}{markers}"),
    );
    if !text.is_empty() {
        let mid = (x1 + x2) / 2.0;
        svg.text(
            mid,
            y - TEXT_OFFSET,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            text,
        );
    }
}

/// Draw the autonumber badge: a filled circle sitting on the arrow origin with
/// the message's sequence number in white, matching upstream's `.sequenceNumber`
/// (replaces the pre-#268 `"1. "` text prefix).
pub(super) fn draw_seq_number(svg: &mut SvgBuilder, x: f64, y: f64, n: f64, theme: &Theme) {
    let fill = &theme.actor_stroke;
    svg.circle(
        x,
        y,
        SEQ_BADGE_R,
        &format!("fill=\"{fill}\" stroke=\"none\""),
    );
    svg.text(
        x,
        y + 4.0,
        "text-anchor=\"middle\" fill=\"#fff\" font-size=\"11\"",
        &fmt_seq_number(n),
    );
}

/// Box geometry for a note: its rectangle plus the text wrapped to fit inside.
pub(super) struct NoteGeom {
    pub rect_x: f64,
    pub rect_w: f64,
    pub height: f64,
    pub lines: Vec<String>,
}

/// Compute a note's box position, width, wrapped text lines, and height.
/// `over` notes span their participants (with a sensible minimum); `left/right
/// of` notes keep a fixed width. Text is word-wrapped to the box interior so
/// long notes grow taller instead of overflowing (issue #123).
pub(super) fn note_geometry(note: &SequenceNote, x_of: &HashMap<String, f64>) -> Option<NoteGeom> {
    if note.participants.is_empty() {
        return None;
    }
    let xs: Vec<f64> = note
        .participants
        .iter()
        .filter_map(|id| x_of.get(id).copied())
        .collect();
    if xs.is_empty() {
        return None;
    }
    let (rect_x, rect_w) = match note.position {
        NotePosition::Over => {
            let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let pad = 20.0;
            let w = ((max_x - min_x) + pad * 2.0).max(NOTE_MIN_W);
            let cx = (min_x + max_x) / 2.0;
            (cx - w / 2.0, w)
        }
        NotePosition::RightOf => (xs[0] + 12.0, NOTE_SIDE_W),
        NotePosition::LeftOf => (xs[0] - 12.0 - NOTE_SIDE_W, NOTE_SIDE_W),
    };
    let lines = wrap_note_text(&note.text, rect_w - NOTE_PAD_X * 2.0);
    let height = (lines.len() as f64 * NOTE_LINE_H + NOTE_PAD_Y * 2.0).max(NOTE_HEIGHT);
    Some(NoteGeom {
        rect_x,
        rect_w,
        height,
        lines,
    })
}

/// Greedy word-wrap `text` to `max_w` pixels, honoring existing `<br>`/`\n`
/// breaks first. Always returns at least one (possibly empty) line.
fn wrap_note_text(text: &str, max_w: f64) -> Vec<String> {
    let max_chars = ((max_w / NOTE_CHAR_W).floor() as usize).max(1);
    let mut out = Vec::new();
    for seg in crate::svg::builder::split_label_lines(text) {
        let mut cur = String::new();
        for word in seg.split_whitespace() {
            if cur.is_empty() {
                cur.push_str(word);
            } else if cur.chars().count() + 1 + word.chars().count() <= max_chars {
                cur.push(' ');
                cur.push_str(word);
            } else {
                out.push(std::mem::take(&mut cur));
                cur.push_str(word);
            }
        }
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

pub(super) fn draw_note(
    svg: &mut SvgBuilder,
    note: &SequenceNote,
    y: f64,
    x_of: &HashMap<String, f64>,
    theme: &Theme,
) {
    let fg = &theme.fg;
    let note_fill = &theme.note_fill;
    let note_stroke = &theme.note_stroke;
    let Some(g) = note_geometry(note, x_of) else {
        return;
    };
    let yy = y - g.height / 2.0;
    svg.rect(
        g.rect_x,
        yy,
        g.rect_w,
        g.height,
        &format!("fill=\"{note_fill}\" stroke=\"{note_stroke}\" stroke-width=\"1\""),
    );
    let cx = g.rect_x + g.rect_w / 2.0;
    let n = g.lines.len() as f64;
    let y0 = y - (n - 1.0) * NOTE_LINE_H / 2.0 + 4.0;
    for (i, line) in g.lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * NOTE_LINE_H,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            line,
        );
    }
}

/// Returns `(dash, start_marker, end_marker)` for an arrow kind. Plain `->`/`-->`
/// are bare lines (no marker); bidirectional arrows carry a marker at both ends.
fn stroke_for(a: ArrowKind) -> (&'static str, Option<&'static str>, Option<&'static str>) {
    match a {
        ArrowKind::Solid => ("", None, None),
        ArrowKind::SolidArrow => ("", None, Some("arrow-filled")),
        ArrowKind::Dashed => ("6 4", None, None),
        ArrowKind::DashedArrow => ("6 4", None, Some("arrow-filled")),
        ArrowKind::Cross => ("", None, Some("arrow-cross")),
        ArrowKind::DashedCross => ("6 4", None, Some("arrow-cross")),
        ArrowKind::Open => ("", None, Some("arrow-open")),
        ArrowKind::DashedOpen => ("6 4", None, Some("arrow-open")),
        ArrowKind::BiSolidArrow => ("", Some("arrow-filled"), Some("arrow-filled")),
        ArrowKind::BiDashedArrow => ("6 4", Some("arrow-filled"), Some("arrow-filled")),
        // Half arrows: the upper (`\`) / lower (`/`) barb picks the marker, and
        // reverse forms hang it off the tail (start) instead of the head (end).
        ArrowKind::HalfArrowTop => ("", None, Some("arrow-half-top")),
        ArrowKind::HalfArrowBottom => ("", None, Some("arrow-half-bottom")),
        ArrowKind::DashedHalfArrowTop => ("6 4", None, Some("arrow-half-top")),
        ArrowKind::DashedHalfArrowBottom => ("6 4", None, Some("arrow-half-bottom")),
        ArrowKind::HalfArrowStartTop => ("", Some("arrow-half-top"), None),
        ArrowKind::HalfArrowStartBottom => ("", Some("arrow-half-bottom"), None),
        ArrowKind::DashedHalfArrowStartTop => ("6 4", Some("arrow-half-top"), None),
        ArrowKind::DashedHalfArrowStartBottom => ("6 4", Some("arrow-half-bottom"), None),
    }
}

pub(super) fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
    let arrow_stroke = &theme.arrow_stroke;
    let h = ARROW_HEAD;
    let filled = format!(
        "<marker id=\"arrow-filled\" viewBox=\"0 0 {h} {h}\" refX=\"{h}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L{h} {half} L0 {h} z\" fill=\"{arrow_stroke}\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let open = format!(
        "<marker id=\"arrow-open\" viewBox=\"0 0 {h} {h}\" refX=\"{h}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L{h} {half} L0 {h}\" fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let cross = format!(
        "<marker id=\"arrow-cross\" viewBox=\"0 0 {h} {h}\" refX=\"{half}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto\">\
         <path d=\"M0 0 L{h} {h} M{h} 0 L0 {h}\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    // Half arrows: a single barb of the arrowhead. `\` is the upper barb
    // (top-left → tip), `/` the lower barb (bottom-left → tip); reverse forms
    // reuse the same markers at the tail.
    let half_top = format!(
        "<marker id=\"arrow-half-top\" viewBox=\"0 0 {h} {h}\" refX=\"{h}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L{h} {half}\" fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let half_bottom = format!(
        "<marker id=\"arrow-half-bottom\" viewBox=\"0 0 {h} {h}\" refX=\"{h}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto-start-reverse\">\
         <path d=\"M0 {h} L{h} {half}\" fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    svg.defs_raw(&filled);
    svg.defs_raw(&open);
    svg.defs_raw(&cross);
    svg.defs_raw(&half_top);
    svg.defs_raw(&half_bottom);
}

fn svg_n(v: f64) -> String {
    crate::svg::builder::fnum(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> SequenceDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Sequence(d) => d,
            _ => panic!("not sequence"),
        }
    }

    #[test]
    fn short_note_stays_single_line_at_default_height() {
        let mut x_of = HashMap::new();
        x_of.insert("B".to_string(), 100.0);
        let note = SequenceNote {
            position: NotePosition::RightOf,
            participants: vec!["B".into()],
            text: "hi".into(),
        };
        let g = note_geometry(&note, &x_of).unwrap();
        assert_eq!(g.lines, vec!["hi".to_string()]);
        assert_eq!(g.height, NOTE_HEIGHT);
    }

    #[test]
    fn long_note_wraps_and_grows_taller() {
        let mut x_of = HashMap::new();
        x_of.insert("B".to_string(), 100.0);
        let note = SequenceNote {
            position: NotePosition::RightOf,
            participants: vec!["B".into()],
            text: "the quick brown fox jumps over the lazy dog again and again".into(),
        };
        let g = note_geometry(&note, &x_of).unwrap();
        assert!(g.lines.len() > 1, "long note must wrap: {:?}", g.lines);
        assert!(g.height > NOTE_HEIGHT, "wrapped note grows taller");
        // No wrapped line exceeds the box interior width.
        let max_chars = ((g.rect_w - NOTE_PAD_X * 2.0) / NOTE_CHAR_W).floor() as usize;
        assert!(g.lines.iter().all(|l| l.chars().count() <= max_chars));
    }

    #[test]
    fn note_honors_explicit_line_breaks() {
        let mut x_of = HashMap::new();
        x_of.insert("A".to_string(), 100.0);
        x_of.insert("B".to_string(), 300.0);
        let note = SequenceNote {
            position: NotePosition::Over,
            participants: vec!["A".into(), "B".into()],
            text: "first<br/>second".into(),
        };
        let g = note_geometry(&note, &x_of).unwrap();
        assert_eq!(g.lines, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn note_uses_theme_fill() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: hi\nNote over A,B: shared\n"),
            &Theme::dark(),
        );
        // Dark theme must not emit the light-yellow default note fill.
        assert!(!svg.contains("#FFF5AD"));
        assert!(svg.contains(Theme::dark().note_fill.as_ref()));
    }

    #[test]
    fn note_over_renders() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: hi\nNote over A,B: shared\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">shared<"));
    }

    #[test]
    fn dashed_cross_and_open_carry_dash() {
        // `--x` / `--)` are dashed; `-x` / `-)` are solid — same marker, but the
        // dashed forms add `stroke-dasharray` (issue #115).
        for (src, marker) in [("A--xB: t", "arrow-cross"), ("A--)B: t", "arrow-open")] {
            let svg = render(
                &build(&format!("sequenceDiagram\n{src}\n")),
                &Theme::default(),
            );
            assert!(svg.contains(&format!("marker-end=\"url(#{marker})\"")));
            assert!(svg.contains("stroke-dasharray=\"6 4\""), "case: {src}");
        }
        for (src, marker) in [("A-xB: t", "arrow-cross"), ("A-)B: t", "arrow-open")] {
            let svg = render(
                &build(&format!("sequenceDiagram\n{src}\n")),
                &Theme::default(),
            );
            assert!(svg.contains(&format!("marker-end=\"url(#{marker})\"")));
            // Lifelines are solid; only a dashed message line carries `6 4`.
            assert!(!svg.contains("stroke-dasharray=\"6 4\""), "case: {src}");
        }
    }
}
