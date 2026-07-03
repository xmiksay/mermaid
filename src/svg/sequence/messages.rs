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
    let fg = theme.fg;
    let arrow_stroke = theme.arrow_stroke;
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

pub(super) fn draw_note(
    svg: &mut SvgBuilder,
    note: &SequenceNote,
    y: f64,
    x_of: &HashMap<String, f64>,
    theme: &Theme,
) {
    let fg = theme.fg;
    if note.participants.is_empty() {
        return;
    }
    let xs: Vec<f64> = note
        .participants
        .iter()
        .filter_map(|id| x_of.get(id).copied())
        .collect();
    if xs.is_empty() {
        return;
    }
    let (rect_x, rect_w) = match note.position {
        NotePosition::Over => {
            let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let pad = 20.0;
            (min_x - pad, (max_x - min_x) + pad * 2.0)
        }
        NotePosition::RightOf => {
            let x = xs[0];
            (x + 12.0, 140.0)
        }
        NotePosition::LeftOf => {
            let x = xs[0];
            (x - 12.0 - 140.0, 140.0)
        }
    };
    let h = NOTE_HEIGHT;
    let yy = y - h / 2.0;
    svg.rect(
        rect_x,
        yy,
        rect_w,
        h,
        "fill=\"#FFF5AD\" stroke=\"#aaaa33\" stroke-width=\"1\"",
    );
    svg.text(
        rect_x + rect_w / 2.0,
        y + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
        &note.text,
    );
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
    }
}

pub(super) fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
    let arrow_stroke = theme.arrow_stroke;
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
    svg.defs_raw(&filled);
    svg.defs_raw(&open);
    svg.defs_raw(&cross);
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
            // Lifelines carry `4 4`; only the message-line dash is `6 4`.
            assert!(!svg.contains("stroke-dasharray=\"6 4\""), "case: {src}");
        }
    }
}
