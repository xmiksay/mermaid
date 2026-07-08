//! ZenUML renderer. Reuses the sequence layout pass but draws ZenUML's own
//! chrome (issue #266): grey activation bars derived from call nesting,
//! hierarchical `1.1.1` message numbers, top-only boxed participants (no bottom
//! actor row), suppressed synthesized returns, a shaded-header fragment style,
//! and a top-left title frame around the whole diagram.

use std::collections::HashMap;

use crate::parse::{ArrowKind, ParticipantKind, SequenceDiagram};
use crate::svg::metrics::text_width;

use super::*;

/// Inset of the enclosing diagram frame from the canvas edge.
const FRAME_INSET: f64 = 8.0;
/// Height of the top-left title tab.
const FRAME_HEADER_H: f64 = 22.0;
/// Gap between the title tab and the participant row.
const FRAME_HEADER_GAP: f64 = 10.0;

pub(super) fn render(d: &SequenceDiagram, theme: &Theme) -> String {
    if d.participants.is_empty() {
        let mut svg = SvgBuilder::new(200.0, 60.0).theme(theme);
        if let Some(t) = &d.title {
            svg.text(
                100.0,
                30.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{}\" font-size=\"16\"",
                    theme.fg
                ),
                t,
            );
        }
        return svg.finish();
    }

    // Column geometry: same measurement as the sequence renderer.
    let sizes: Vec<(f64, f64)> = d
        .participants
        .iter()
        .map(|p| actor_size(&p.display, theme.font_size))
        .collect();
    let actor_h = sizes.iter().map(|s| s.1).fold(ACTOR_H, f64::max);

    let mut x_of: HashMap<String, f64> = HashMap::new();
    let mut x = PAD;
    for (i, p) in d.participants.iter().enumerate() {
        let w = sizes[i].0;
        x_of.insert(p.id.clone(), x + w / 2.0);
        x += w + PARTICIPANT_GAP;
    }
    let width = x - PARTICIPANT_GAP + PAD;

    let header_tab_h = if d.title.is_some() {
        FRAME_HEADER_H
    } else {
        0.0
    };
    let top_y = FRAME_INSET + header_tab_h + FRAME_HEADER_GAP;
    let header_bottom = top_y + actor_h;
    let body_top = header_bottom + MSG_TOP_GAP;

    // Layout pass (shared): collects message/activation/frame events with y.
    let mut events: Vec<Event> = Vec::new();
    let mut cursor = body_top;
    let mut counter = 1.0;
    let mut num = Numbering {
        on: false,
        step: 1.0,
    };
    layout_items(
        &d.items,
        &mut events,
        &mut cursor,
        &mut counter,
        &mut num,
        &x_of,
    );
    let lifeline_bottom = cursor + MSG_BOTTOM_GAP;
    let frame_bottom = lifeline_bottom + FRAME_HEADER_GAP;
    let height = frame_bottom + FRAME_INSET;

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    define_markers(&mut svg, theme);

    // Enclosing frame with a top-left title tab.
    draw_frame(&mut svg, width, frame_bottom, d.title.as_deref(), theme);

    // Solid lifelines (ZenUML draws bars over solid lines, not dashed ones).
    for p in &d.participants {
        let cx = x_of[&p.id];
        svg.line(
            cx,
            header_bottom,
            cx,
            lifeline_bottom,
            &format!("stroke=\"{}\" stroke-width=\"1\"", theme.lifeline),
        );
    }

    draw_activations(&mut svg, &events, &x_of, lifeline_bottom, theme);
    draw_block_frames(&mut svg, &events, &x_of, theme, true);

    // Top-only boxed participants (no bottom actor row).
    for (i, p) in d.participants.iter().enumerate() {
        draw_participant(
            &mut svg,
            x_of[&p.id],
            top_y,
            sizes[i].0,
            actor_h,
            &p.display,
            p.kind,
            theme,
        );
    }

    // Hierarchical `1.1.1` numbers over the call tree.
    let numbers = number_calls(&events);

    for (i, ev) in events.iter().enumerate() {
        match &ev.kind {
            EventKind::Message { msg, .. } => {
                if let (Some(&x1), Some(&x2)) = (x_of.get(&msg.from), x_of.get(&msg.to)) {
                    let label = match numbers.get(&i) {
                        Some(n) if !msg.text.is_empty() => format!("{n} {}", msg.text),
                        Some(n) => n.clone(),
                        None => msg.text.clone(),
                    };
                    draw_message(&mut svg, x1, x2, ev.y, msg.arrow, &label, theme);
                }
            }
            EventKind::Note(note) => draw_note(&mut svg, note, ev.y, &x_of, theme),
            _ => {}
        }
    }

    svg.finish()
}

/// Assign each forward call (solid arrow) an outline number keyed by its event
/// index. Depth follows the activation stack: a call increments the counter at
/// its level, and its `Activate` pushes a fresh child level. Returns (dashed)
/// are not numbered.
fn number_calls(events: &[Event]) -> HashMap<usize, String> {
    let mut stack: Vec<u32> = vec![0];
    let mut out = HashMap::new();
    for (i, ev) in events.iter().enumerate() {
        match &ev.kind {
            EventKind::Message { msg, .. }
                if matches!(msg.arrow, ArrowKind::Solid | ArrowKind::SolidArrow) =>
            {
                if let Some(last) = stack.last_mut() {
                    *last += 1;
                }
                out.insert(
                    i,
                    stack
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join("."),
                );
            }
            EventKind::Activate(_) => stack.push(0),
            EventKind::Deactivate(_) if stack.len() > 1 => {
                stack.pop();
            }
            _ => {}
        }
    }
    out
}

fn draw_frame(
    svg: &mut SvgBuilder,
    width: f64,
    frame_bottom: f64,
    title: Option<&str>,
    theme: &Theme,
) {
    let x = FRAME_INSET;
    let w = width - FRAME_INSET * 2.0;
    svg.rect(
        x,
        FRAME_INSET,
        w,
        frame_bottom - FRAME_INSET,
        "fill=\"none\" stroke=\"#666\" stroke-width=\"1\"",
    );
    if let Some(t) = title {
        let tab_w = (text_width(t, 8.0, theme.font_size) + 20.0).min(w);
        svg.rect(
            x,
            FRAME_INSET,
            tab_w,
            FRAME_HEADER_H,
            &format!(
                "fill=\"{}\" stroke=\"#666\" stroke-width=\"1\"",
                theme.frame_label_fill
            ),
        );
        svg.text(
            x + 8.0,
            FRAME_INSET + 15.0,
            &format!(
                "fill=\"{}\" font-size=\"12\" font-weight=\"bold\"",
                theme.fg
            ),
            t,
        );
    }
}

/// A top-only ZenUML participant: a bordered box carrying the stereotype icon
/// (or actor figure) and the name. Plain participants are just the boxed name.
#[allow(clippy::too_many_arguments)]
fn draw_participant(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    kind: ParticipantKind,
    theme: &Theme,
) {
    if !matches!(kind, ParticipantKind::Participant) {
        // Frame the stereotype/actor glyph in a border box (ZenUML chrome).
        svg.rect(
            cx - w / 2.0,
            top,
            w,
            h,
            &format!(
                "fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" rx=\"2\"",
                theme.actor_fill, theme.actor_stroke
            ),
        );
    }
    draw_actor(svg, cx, top, w, h, label, kind, theme);
}

#[cfg(test)]
mod tests {
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
        // Header band names the operator + condition; else region is shaded.
        assert!(svg.contains(">alt [found]<"), "shaded header band");
        assert!(svg.contains("rgba(0,0,0,0.04)"), "shaded else region");
    }

    #[test]
    fn emits_hierarchical_numbers() {
        let svg = render(&build(SAMPLE), &Theme::default());
        assert!(svg.contains("1 login(name, pass)"), "top call is 1");
        assert!(svg.contains("1.1 verify(name, pass)"), "nested call is 1.1");
        assert!(svg.contains("1.1.1 query(name)"), "deep call is 1.1.1");
        assert!(svg.contains("1.2 render()"), "sibling call is 1.2");
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
}
