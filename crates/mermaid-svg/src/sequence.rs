//! Sequence diagram renderer.
//!
//! Layout (no sugiyama, since sequence diagrams are inherently 1-D):
//!   * Participants laid out left-to-right with fixed spacing.
//!   * Each gets a header box at the top and a lifeline going down.
//!   * Messages are horizontal arrows at increasing y positions.

use std::collections::HashMap;

use mermaid_parse::{ArrowKind, SequenceDiagram};

use crate::svg::SvgBuilder;
use crate::theme::{ACTOR_FILL, ACTOR_STROKE, ARROW_STROKE, FG, LIFELINE};

const ACTOR_W: f64 = 100.0;
const ACTOR_H: f64 = 40.0;
const PARTICIPANT_GAP: f64 = 60.0;
const PAD: f64 = 24.0;
const TITLE_H: f64 = 30.0;
const MSG_STEP: f64 = 50.0;
const MSG_TOP_GAP: f64 = 30.0;
const MSG_BOTTOM_GAP: f64 = 30.0;
const ARROW_HEAD: f64 = 8.0;
const TEXT_OFFSET: f64 = 6.0;

pub(crate) fn render(d: &SequenceDiagram) -> String {
    if d.participants.is_empty() {
        // No participants → empty canvas with title.
        let mut svg = SvgBuilder::new(200.0, 60.0);
        if let Some(t) = &d.title {
            svg.text(
                100.0,
                30.0,
                &format!("text-anchor=\"middle\" fill=\"{FG}\" font-size=\"16\""),
                t,
            );
        }
        return svg.finish();
    }

    let title_h = if d.title.is_some() { TITLE_H } else { 0.0 };

    // Participant x positions (center of header box).
    let mut x_of: HashMap<String, f64> = HashMap::new();
    for (i, p) in d.participants.iter().enumerate() {
        let x = PAD + ACTOR_W / 2.0 + (i as f64) * (ACTOR_W + PARTICIPANT_GAP);
        x_of.insert(p.id.clone(), x);
    }
    let last_x = PAD + (d.participants.len() as f64) * ACTOR_W
        + (d.participants.len().saturating_sub(1) as f64) * PARTICIPANT_GAP;
    let width = last_x + PAD;

    let top_y = PAD + title_h;
    let header_bottom = top_y + ACTOR_H;
    let body_top = header_bottom + MSG_TOP_GAP;
    let body_height = d.messages.len() as f64 * MSG_STEP;
    let lifeline_bottom = body_top + body_height + MSG_BOTTOM_GAP;
    let footer_top = lifeline_bottom;
    let footer_bottom = footer_top + ACTOR_H;
    let height = footer_bottom + PAD;

    let mut svg = SvgBuilder::new(width, height);
    define_markers(&mut svg);

    // Title
    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{FG}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    // Lifelines (drawn first so participant boxes overlap on top)
    for p in &d.participants {
        let x = x_of[&p.id];
        svg.line(
            x,
            header_bottom,
            x,
            lifeline_bottom,
            &format!("stroke=\"{LIFELINE}\" stroke-width=\"1\" stroke-dasharray=\"4 4\""),
        );
    }

    // Participant headers (top and bottom)
    for p in &d.participants {
        let x = x_of[&p.id];
        draw_actor(&mut svg, x, top_y, &p.display);
        draw_actor(&mut svg, x, footer_top, &p.display);
    }

    // Messages
    for (i, m) in d.messages.iter().enumerate() {
        let y = body_top + (i as f64) * MSG_STEP + MSG_STEP / 2.0;
        let Some(&x1) = x_of.get(&m.from) else {
            continue;
        };
        let Some(&x2) = x_of.get(&m.to) else {
            continue;
        };
        draw_message(&mut svg, x1, x2, y, m.arrow, &m.text);
    }

    svg.finish()
}

fn draw_actor(svg: &mut SvgBuilder, cx: f64, top: f64, label: &str) {
    let x = cx - ACTOR_W / 2.0;
    svg.rect(
        x,
        top,
        ACTOR_W,
        ACTOR_H,
        &format!("fill=\"{ACTOR_FILL}\" stroke=\"{ACTOR_STROKE}\" stroke-width=\"1.5\" rx=\"4\""),
    );
    svg.text(
        cx,
        top + ACTOR_H / 2.0 + 5.0,
        &format!("text-anchor=\"middle\" fill=\"{FG}\""),
        label,
    );
}

fn draw_message(svg: &mut SvgBuilder, x1: f64, x2: f64, y: f64, arrow: ArrowKind, text: &str) {
    let (dash, marker) = stroke_for(arrow);
    let dash_attr = if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{dash}\"")
    };

    if (x1 - x2).abs() < 1e-6 {
        // Self-message: draw a small rounded path that loops to the right.
        let r = 20.0;
        let d_attr = format!(
            "M{x1} {y}L{x_right} {y}L{x_right} {y2}L{x1} {y2}",
            x1 = svg_n(x1),
            y = svg_n(y),
            x_right = svg_n(x1 + r),
            y2 = svg_n(y + r),
        );
        svg.path(
            &d_attr,
            &format!(
                "fill=\"none\" stroke=\"{ARROW_STROKE}\" stroke-width=\"1.5\"{dash_attr} marker-end=\"url(#{marker})\""
            ),
        );
        if !text.is_empty() {
            svg.text(
                x1 + r + 4.0,
                y + r / 2.0 + 4.0,
                &format!("fill=\"{FG}\" font-size=\"12\""),
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
        &format!(
            "stroke=\"{ARROW_STROKE}\" stroke-width=\"1.5\"{dash_attr} marker-end=\"url(#{marker})\""
        ),
    );
    if !text.is_empty() {
        let mid = (x1 + x2) / 2.0;
        svg.text(
            mid,
            y - TEXT_OFFSET,
            &format!("text-anchor=\"middle\" fill=\"{FG}\" font-size=\"12\""),
            text,
        );
    }
}

fn stroke_for(a: ArrowKind) -> (&'static str, &'static str) {
    match a {
        ArrowKind::Solid => ("", "arrow-open"),
        ArrowKind::SolidArrow => ("", "arrow-filled"),
        ArrowKind::Dashed => ("6 4", "arrow-open"),
        ArrowKind::DashedArrow => ("6 4", "arrow-filled"),
        ArrowKind::Cross => ("", "arrow-cross"),
        ArrowKind::Open => ("", "arrow-open"),
    }
}

fn define_markers(svg: &mut SvgBuilder) {
    let h = ARROW_HEAD;
    let filled = format!(
        "<marker id=\"arrow-filled\" viewBox=\"0 0 {h} {h}\" \
         refX=\"{h}\" refY=\"{half}\" markerWidth=\"{h}\" markerHeight=\"{h}\" \
         orient=\"auto-start-reverse\"><path d=\"M0 0 L{h} {half} L0 {h} z\" \
         fill=\"{ARROW_STROKE}\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let open = format!(
        "<marker id=\"arrow-open\" viewBox=\"0 0 {h} {h}\" \
         refX=\"{h}\" refY=\"{half}\" markerWidth=\"{h}\" markerHeight=\"{h}\" \
         orient=\"auto-start-reverse\"><path d=\"M0 0 L{h} {half} L0 {h}\" \
         fill=\"none\" stroke=\"{ARROW_STROKE}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let cross = format!(
        "<marker id=\"arrow-cross\" viewBox=\"0 0 {h} {h}\" \
         refX=\"{half}\" refY=\"{half}\" markerWidth=\"{h}\" markerHeight=\"{h}\" \
         orient=\"auto\"><path d=\"M0 0 L{h} {h} M{h} 0 L0 {h}\" \
         stroke=\"{ARROW_STROKE}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    svg.defs_raw(&filled);
    svg.defs_raw(&open);
    svg.defs_raw(&cross);
}

fn svg_n(v: f64) -> String {
    crate::svg::fnum(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_parse::{Message, Participant, ParticipantKind};

    fn pid(s: &str) -> Participant {
        Participant {
            id: s.into(),
            display: s.into(),
            kind: ParticipantKind::Participant,
        }
    }

    fn msg(from: &str, to: &str, arrow: ArrowKind, text: &str) -> Message {
        Message {
            from: from.into(),
            to: to.into(),
            text: text.into(),
            arrow,
        }
    }

    #[test]
    fn basic_envelope() {
        let d = SequenceDiagram {
            title: Some("Login".into()),
            participants: vec![pid("alice"), pid("bob")],
            messages: vec![msg("alice", "bob", ArrowKind::SolidArrow, "hi")],
        };
        let svg = render(&d);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Login"));
        assert!(svg.contains("alice"));
        assert!(svg.contains("bob"));
        assert!(svg.contains("hi"));
        assert!(svg.contains("arrow-filled"));
    }

    #[test]
    fn dashed_renders_dasharray() {
        let d = SequenceDiagram {
            title: None,
            participants: vec![pid("a"), pid("b")],
            messages: vec![msg("a", "b", ArrowKind::Dashed, "x")],
        };
        let svg = render(&d);
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains("arrow-open"));
    }

    #[test]
    fn cross_marker_used() {
        let d = SequenceDiagram {
            title: None,
            participants: vec![pid("a"), pid("b")],
            messages: vec![msg("a", "b", ArrowKind::Cross, "")],
        };
        let svg = render(&d);
        assert!(svg.contains("arrow-cross"));
    }

    #[test]
    fn self_message_renders() {
        let d = SequenceDiagram {
            title: None,
            participants: vec![pid("a")],
            messages: vec![msg("a", "a", ArrowKind::SolidArrow, "loop")],
        };
        let svg = render(&d);
        assert!(svg.contains("loop"));
        // Self message uses a path, not a line
        assert!(svg.contains("<path"));
    }

    #[test]
    fn xml_text_is_escaped() {
        let d = SequenceDiagram {
            title: None,
            participants: vec![pid("a"), pid("b")],
            messages: vec![msg("a", "b", ArrowKind::Solid, "x < y & z")],
        };
        let svg = render(&d);
        assert!(svg.contains("x &lt; y &amp; z"));
    }
}
