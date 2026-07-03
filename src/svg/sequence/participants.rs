//! `box` grouping backgrounds, activation bands, and the `destroy` termination
//! cross — the lifeline decorations. Participant/actor header glyphs live in
//! [`super::glyphs`].

use std::collections::HashMap;

use super::*;

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
    let fg = &theme.fg;
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

pub(super) fn draw_activations(
    svg: &mut SvgBuilder,
    events: &[Event],
    x_of: &HashMap<String, f64>,
    lifeline_bottom: f64,
    theme: &Theme,
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
                    draw_activation_band(svg, x_of, id, start_y, ev.y, level, theme);
                }
            }
            _ => {}
        }
    }
    // Unclosed activations extend to the bottom of the lifelines.
    for (id, starts) in &open {
        for (level, &start_y) in starts.iter().enumerate() {
            draw_activation_band(svg, x_of, id, start_y, lifeline_bottom, level, theme);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_activation_band(
    svg: &mut SvgBuilder,
    x_of: &HashMap<String, f64>,
    id: &str,
    start_y: f64,
    end_y: f64,
    level: usize,
    theme: &Theme,
) {
    let activation_fill = &theme.activation_fill;
    let activation_stroke = &theme.activation_stroke;
    if let Some(&cx) = x_of.get(id) {
        let offset = level as f64 * 3.0;
        svg.rect(
            cx - ACTIVATION_W / 2.0 + offset,
            start_y,
            ACTIVATION_W,
            (end_y - start_y).max(8.0),
            &format!(
                "fill=\"{activation_fill}\" stroke=\"{activation_stroke}\" stroke-width=\"1\""
            ),
        );
    }
}

/// Draw the `×` that terminates a destroyed participant's lifeline.
pub(super) fn draw_destroy_cross(svg: &mut SvgBuilder, cx: f64, cy: f64, theme: &Theme) {
    let stroke = &theme.arrow_stroke;
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
    fn activation_uses_theme_fill() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: req\nactivate B\nB-->>A: resp\ndeactivate B\n"),
            &Theme::dark(),
        );
        // Dark theme must not fall back to the default light activation fill.
        assert!(!svg.contains("#ECECFF"));
        assert!(svg.contains(Theme::dark().activation_fill.as_ref()));
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
        draw_activations(&mut svg, &events, &x_of, 200.0, &Theme::default());
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
        draw_activations(&mut svg, &events, &x_of, 200.0, &Theme::default());
        let out = svg.finish();
        // Band height = lifeline_bottom - start_y = 200 - 10 = 190.
        assert!(out.contains("#ECECFF"), "unclosed activation is drawn");
        assert!(out.contains("height=\"190\""), "extends to lifeline bottom");
    }
}
