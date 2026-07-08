//! ZenUML renderer. Reuses the sequence layout pass but draws ZenUML's own
//! chrome (issues #266, #315): grey activation bars derived from call nesting,
//! hierarchical `1.1.1` message numbers dimmed gray (returns included), top-only
//! white/gray participant boxes (icon left of the name, no bottom actor row),
//! suppressed synthesized returns, a `◇ Operator` fragment header, and a
//! top-left title frame around the whole diagram.

use std::collections::HashMap;

use crate::parse::{ParticipantKind, SequenceDiagram};
use crate::svg::metrics::text_width;

use super::*;

/// Inset of the enclosing diagram frame from the canvas edge.
const FRAME_INSET: f64 = 8.0;
/// Height of the top-left title tab.
const FRAME_HEADER_H: f64 = 22.0;
/// Gap between the title tab and the participant row.
const FRAME_HEADER_GAP: f64 = 10.0;
/// Width of the left icon column inside a stereotype/actor participant box.
const ICON_W: f64 = 26.0;

/// A ZenUML participant box's `(width, height)`. It reuses the sequence
/// `actor_size` measurement and reserves a left icon column for the
/// stereotype/actor glyph (which sits beside the name, not above it).
fn participant_size(label: &str, kind: ParticipantKind, font_size: f64) -> (f64, f64) {
    let (w, h) = actor_size(label, font_size);
    if matches!(kind, ParticipantKind::Participant) {
        (w, h)
    } else {
        (w + ICON_W, h)
    }
}

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

    // Column geometry: sequence measurement plus a left icon column.
    let sizes: Vec<(f64, f64)> = d
        .participants
        .iter()
        .map(|p| participant_size(&p.display, p.kind, theme.font_size))
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
                    // The sequence number is dimmed gray (upstream `.number`); the
                    // gray run is emitted as an inline `<span>` so the label text
                    // keeps the default color beside it.
                    let num =
                        |n: &str| format!("<span style=\"color:{}\">{n}</span>", theme.fg_muted);
                    let label = match numbers.get(&i) {
                        Some(n) if !msg.text.is_empty() => format!("{} {}", num(n), msg.text),
                        Some(n) => num(n),
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

/// Assign every message an outline number keyed by its event index. Depth
/// follows the call tree: each message increments the counter at the current
/// level, an `Activate` pushes a fresh child level, and a fragment (`BlockOpen`)
/// counts as a step and opens its own level so its returns nest under it.
///
/// A call's own reply (an `x = A.m()` assignment return) is emitted *after* the
/// receiver's `Deactivate`, so it is numbered as the last child of the call's
/// frame — recognised as the dashed message right after `Deactivate(id)` whose
/// `from` is `id`. Explicit `return`s sit inside the body (before the
/// deactivate) and number naturally at their own level. This yields upstream
/// ZenUML's `1`, `1.1`, `1.1.1`, `1.1.1.1 found`, `1.1.2.1 token`,
/// `1.1.2.2 denied`, `1.2`.
fn number_calls(events: &[Event]) -> HashMap<usize, String> {
    let mut stack: Vec<u32> = vec![0];
    let mut out = HashMap::new();
    for (i, ev) in events.iter().enumerate() {
        match &ev.kind {
            // Already numbered as its call's reply during the `Deactivate` below.
            EventKind::Message { .. } if out.contains_key(&i) => {}
            EventKind::Message { .. } => {
                let n = bump(&mut stack);
                out.insert(i, n);
            }
            EventKind::Activate(_) => stack.push(0),
            // A fragment is itself a numbered step; its own level holds the
            // returns drawn inside its branches.
            EventKind::BlockOpen { .. } => {
                bump(&mut stack);
                stack.push(0);
            }
            EventKind::Deactivate(id) if stack.len() > 1 => {
                if let Some((ret_i, EventKind::Message { msg, .. })) =
                    events.get(i + 1).map(|e| (i + 1, &e.kind))
                {
                    if msg.from == *id {
                        let n = bump(&mut stack);
                        out.insert(ret_i, n);
                    }
                }
                stack.pop();
            }
            EventKind::BlockClose if stack.len() > 1 => {
                stack.pop();
            }
            _ => {}
        }
    }
    out
}

/// Increment the counter at the deepest level and return the dotted path.
fn bump(stack: &mut [u32]) -> String {
    if let Some(last) = stack.last_mut() {
        *last += 1;
    }
    stack
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
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

/// A top-only ZenUML participant: a white, gray-bordered box (issue #315). A
/// stereotype/actor glyph sits in a left icon column with the name centered in
/// the remaining space; a plain participant is just the centered name.
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
    let left = cx - w / 2.0;
    // White fill with a gray border (upstream ZenUML), not the lavender actor box.
    svg.rect(
        left,
        top,
        w,
        h,
        &format!(
            "fill=\"{}\" stroke=\"#666\" stroke-width=\"1\" rx=\"2\"",
            theme.bg
        ),
    );
    let name_cx = if matches!(kind, ParticipantKind::Participant) {
        cx
    } else {
        // Icon on the left, name centered in the space to its right.
        draw_participant_icon(svg, left + ICON_W / 2.0, top + h / 2.0, kind, theme);
        left + ICON_W + (w - ICON_W) / 2.0
    };
    draw_box_name(svg, name_cx, top, h, label, theme);
}

/// Draw a participant's name lines vertically centered in a box of height `h`.
fn draw_box_name(svg: &mut SvgBuilder, cx: f64, top: f64, h: f64, label: &str, theme: &Theme) {
    let fg = theme.actor_text();
    let lines = label_lines(label);
    let n = lines.len() as f64;
    let y0 = top + h / 2.0 - (n - 1.0) * ACTOR_LINE_H / 2.0 + 5.0;
    for (i, line) in lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * ACTOR_LINE_H,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
    }
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
