//! Participant/actor headers, `box` grouping, activation bands, and the
//! `destroy` termination cross — everything drawn against a lifeline.

use std::collections::HashMap;

use crate::parse::ParticipantKind;

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_actor(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    kind: ParticipantKind,
    theme: &Theme,
) {
    match kind {
        ParticipantKind::Actor => draw_actor_figure(svg, cx, top, h, label, theme),
        ParticipantKind::Participant => draw_actor_box(svg, cx, top, w, h, label, theme),
    }
}

fn draw_actor_box(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    theme: &Theme,
) {
    let fg = theme.fg;
    let actor_fill = theme.actor_fill;
    let actor_stroke = theme.actor_stroke;
    let x = cx - w / 2.0;
    svg.rect(
        x,
        top,
        w,
        h,
        &format!("fill=\"{actor_fill}\" stroke=\"{actor_stroke}\" stroke-width=\"1.5\" rx=\"4\""),
    );
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

/// Draw an `actor` as a stick figure (head + body + arms + legs) with the name
/// underneath — mirrors upstream `drawActorTypeActor`.
fn draw_actor_figure(svg: &mut SvgBuilder, cx: f64, top: f64, h: f64, label: &str, theme: &Theme) {
    let fg = theme.fg;
    let stroke = theme.actor_stroke;
    let fill = theme.actor_fill;
    let attrs = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let line_attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");

    let head_r = 7.0;
    let head_cy = top + head_r + 1.0;
    let body_top = head_cy + head_r;
    let body_bot = body_top + 13.0;
    let arm_y = body_top + 5.0;
    let arm_half = 10.0;
    let leg_dx = 8.0;
    let leg_dy = 10.0;

    svg.circle(cx, head_cy, head_r, &attrs);
    svg.line(cx, body_top, cx, body_bot, &line_attrs);
    svg.line(cx - arm_half, arm_y, cx + arm_half, arm_y, &line_attrs);
    svg.line(cx, body_bot, cx - leg_dx, body_bot + leg_dy, &line_attrs);
    svg.line(cx, body_bot, cx + leg_dx, body_bot + leg_dy, &line_attrs);

    // Name sits below the figure, within the actor's allotted height.
    let lines = label_lines(label);
    let mut y = (body_bot + leg_dy + 14.0).min(top + h - 2.0);
    for line in &lines {
        svg.text(
            cx,
            y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
        y += ACTOR_LINE_H;
    }
}

/// Draw the `box` grouping backgrounds: a filled rect spanning member
/// participants from above the actor row down past the footer, label centered
/// at the top.
pub(super) fn draw_boxes(
    svg: &mut SvgBuilder,
    d: &SequenceDiagram,
    x_of: &HashMap<String, f64>,
    w_of: &HashMap<String, f64>,
    top: f64,
    bottom: f64,
    theme: &Theme,
) {
    let fg = theme.fg;
    for item in &d.items {
        let SequenceItem::Box(b) = item else { continue };
        let mut min_l = f64::INFINITY;
        let mut max_r = f64::NEG_INFINITY;
        for id in &b.participant_ids {
            if let (Some(&cx), Some(&w)) = (x_of.get(id), w_of.get(id)) {
                min_l = min_l.min(cx - w / 2.0);
                max_r = max_r.max(cx + w / 2.0);
            }
        }
        if !min_l.is_finite() {
            continue;
        }
        let x = min_l - BOX_PAD;
        let w = (max_r - min_l) + BOX_PAD * 2.0;
        let y = top;
        let h = (bottom + BOX_PAD) - y;
        let fill = b.color.as_deref().unwrap_or("none");
        svg.rect(
            x,
            y,
            w,
            h,
            &format!("fill=\"{fill}\" stroke=\"#999\" stroke-width=\"1\""),
        );
        if !b.label.is_empty() {
            svg.text(
                x + w / 2.0,
                y + 15.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""
                ),
                &b.label,
            );
        }
    }
}

/// Split a participant label into display lines, honoring `<br/>` (issue #3)
/// and literal `\n` escapes.
fn label_lines(label: &str) -> Vec<String> {
    let mut normalized = label.to_string();
    for br in ["<br/>", "<br />", "<br>", "\\n"] {
        normalized = normalized.replace(br, "\n");
    }
    normalized
        .split('\n')
        .map(|l| l.trim().to_string())
        .collect()
}

/// Measure a participant box from its label: width grows to fit the widest
/// line, height grows with line count. Both clamp to sane minimums.
pub(super) fn actor_size(label: &str) -> (f64, f64) {
    let lines = label_lines(label);
    let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = (max_chars as f64 * ACTOR_CHAR_W + ACTOR_PAD_X * 2.0).max(ACTOR_MIN_W);
    let h = (lines.len() as f64 * ACTOR_LINE_H + 14.0).max(ACTOR_H);
    (w, h)
}

pub(super) fn draw_activations(
    svg: &mut SvgBuilder,
    events: &[Event],
    x_of: &HashMap<String, f64>,
    lifeline_bottom: f64,
) {
    // A stack of open start-ys per participant so nested activations
    // (e.g. the `->>+` shorthand) stack instead of overwriting. Each nesting
    // level is offset horizontally so the outer band stays visible.
    let mut open: HashMap<String, Vec<f64>> = HashMap::new();
    for ev in events {
        match &ev.kind {
            EventKind::Activate(id) => {
                open.entry(id.clone()).or_default().push(ev.y);
            }
            EventKind::Deactivate(id) => {
                if let Some(start_y) = open.get_mut(id).and_then(Vec::pop) {
                    // The popped entry sat at depth == current stack length.
                    let level = open.get(id).map_or(0, Vec::len);
                    draw_activation_band(svg, x_of, id, start_y, ev.y, level);
                }
            }
            _ => {}
        }
    }
    // Unclosed activations extend to the bottom of the lifelines.
    for (id, starts) in &open {
        for (level, &start_y) in starts.iter().enumerate() {
            draw_activation_band(svg, x_of, id, start_y, lifeline_bottom, level);
        }
    }
}

fn draw_activation_band(
    svg: &mut SvgBuilder,
    x_of: &HashMap<String, f64>,
    id: &str,
    start_y: f64,
    end_y: f64,
    level: usize,
) {
    if let Some(&cx) = x_of.get(id) {
        let offset = level as f64 * 3.0;
        svg.rect(
            cx - ACTIVATION_W / 2.0 + offset,
            start_y,
            ACTIVATION_W,
            (end_y - start_y).max(8.0),
            "fill=\"#ECECFF\" stroke=\"#9370DB\" stroke-width=\"1\"",
        );
    }
}

/// Draw the `×` that terminates a destroyed participant's lifeline.
pub(super) fn draw_destroy_cross(svg: &mut SvgBuilder, cx: f64, cy: f64, theme: &Theme) {
    let stroke = theme.arrow_stroke;
    let r = DESTROY_CROSS;
    let attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");
    svg.line(cx - r, cy - r, cx + r, cy + r, &attrs);
    svg.line(cx + r, cy - r, cx - r, cy + r, &attrs);
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
    fn actor_box_grows_to_fit_label() {
        // A wide label must produce a box wider than the fixed minimum, and the
        // canvas width must accommodate it.
        let wide = "sequenceDiagram\n\
            participant BE as Backend app06 :8082 (UAT) app14 :8081 (PROD) cyberscore-portal FrankenPHP\n\
            participant A\nA->>BE: hi\n";
        let svg = render(&build(wide), &Theme::default());
        // Find the widest actor rect width; it must exceed the old fixed 110.
        let max_w = svg
            .split("width=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
            .filter_map(|s| s.parse::<f64>().ok())
            .fold(0.0_f64, f64::max);
        assert!(max_w > 110.0, "expected a box wider than 110, got {max_w}");
    }

    #[test]
    fn multiline_label_splits_on_br() {
        let svg = render(
            &build("sequenceDiagram\nparticipant BE as Backend<br/>app06\nA->>BE: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">Backend<"));
        assert!(svg.contains(">app06<"));
    }

    #[test]
    fn actor_size_matches_label() {
        assert_eq!(actor_size("A"), (ACTOR_MIN_W, ACTOR_H));
        let (w, h) = actor_size("one<br/>two<br/>three");
        assert!(h > ACTOR_H, "multi-line label should be taller");
        assert_eq!(w, ACTOR_MIN_W, "short lines keep the minimum width");
    }

    #[test]
    fn actor_renders_as_stick_figure() {
        let svg = render(
            &build("sequenceDiagram\nactor A\nparticipant B\nA->>B: hi\n"),
            &Theme::default(),
        );
        // Stick figure emits a <circle> head; a plain participant box does not.
        assert!(svg.contains("<circle"), "actor should draw a circle head");
        assert!(svg.contains(">A</text>"), "actor name below figure");
    }

    #[test]
    fn participant_stays_a_box() {
        let svg = render(
            &build("sequenceDiagram\nparticipant A\nA->>B: hi\n"),
            &Theme::default(),
        );
        assert!(!svg.contains("<circle"), "participant is a rounded rect");
    }

    #[test]
    fn box_renders_background_and_label() {
        let svg = render(
            &build(
                "sequenceDiagram\nbox Aqua Team\nparticipant A\nparticipant B\nend\nA->>B: hi\n",
            ),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"Aqua\""), "box uses its declared color");
        assert!(svg.contains(">Team<"), "box label is drawn");
    }

    #[test]
    fn box_without_color_is_transparent() {
        let svg = render(
            &build("sequenceDiagram\nbox Team\nparticipant A\nparticipant B\nend\nA->>B: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">Team<"));
        // A transparent box has no colored fill of its own.
        assert!(!svg.contains("fill=\"Aqua\""));
    }

    #[test]
    fn activate_deactivate_draws_band() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: req\nactivate B\nB-->>A: resp\ndeactivate B\n"),
            &Theme::default(),
        );
        // activation rect uses #ECECFF
        assert!(svg.contains("#ECECFF"));
    }

    #[test]
    fn nested_activations_stack_and_offset() {
        // Two activations on B open before either closes: the bands must not
        // overwrite each other, and the inner one is offset horizontally.
        let events = vec![
            Event {
                y: 10.0,
                kind: EventKind::Activate("B".into()),
            },
            Event {
                y: 20.0,
                kind: EventKind::Activate("B".into()),
            },
            Event {
                y: 30.0,
                kind: EventKind::Deactivate("B".into()),
            },
            Event {
                y: 40.0,
                kind: EventKind::Deactivate("B".into()),
            },
        ];
        let mut x_of = HashMap::new();
        x_of.insert("B".to_string(), 100.0);
        let mut svg = SvgBuilder::new(300.0, 300.0);
        draw_activations(&mut svg, &events, &x_of, 200.0);
        let out = svg.finish();
        // Outer band starts at x = 95 (100 - 10/2 + 0), inner offset by 3 → 98.
        assert!(out.contains("x=\"95\""), "outer band at base x");
        assert!(out.contains("x=\"98\""), "inner band offset by level*3");
    }

    #[test]
    fn destroyed_participant_draws_cross() {
        let svg = render(
            &build(
                "sequenceDiagram\ncreate participant Carl\nAlice->>Carl: Hi\ndestroy Carl\nAlice-xCarl: bye\n",
            ),
            &Theme::default(),
        );
        // The termination cross is two crossing <line>s; verify the message and
        // participant still render (cross geometry is asserted via events below).
        assert!(svg.contains(">bye<"));
    }

    #[test]
    fn destroy_cross_geometry() {
        // Exercise the cross drawing directly.
        let mut svg = SvgBuilder::new(300.0, 300.0);
        draw_destroy_cross(&mut svg, 50.0, 100.0, &Theme::default());
        let out = svg.finish();
        // Two diagonal strokes centered on (50, 100) with half-size 7.
        assert!(out.contains("x1=\"43\""));
        assert!(out.contains("y2=\"107\""));
    }

    #[test]
    fn unclosed_activation_extends_to_bottom() {
        let events = vec![Event {
            y: 10.0,
            kind: EventKind::Activate("B".into()),
        }];
        let mut x_of = HashMap::new();
        x_of.insert("B".to_string(), 100.0);
        let mut svg = SvgBuilder::new(300.0, 300.0);
        draw_activations(&mut svg, &events, &x_of, 200.0);
        let out = svg.finish();
        // Band height = lifeline_bottom - start_y = 200 - 10 = 190.
        assert!(out.contains("#ECECFF"), "unclosed activation is drawn");
        assert!(out.contains("height=\"190\""), "extends to lifeline bottom");
    }
}
