//! Timeline renderer. Matches upstream Mermaid's block-and-arrow design: a
//! saturated section band tops each group, filled period boxes sit above a thick
//! arrow axis, filled event boxes hang below, and a dashed connector runs from
//! each period through the axis down to its events. The axis is horizontal by
//! default; `direction TB`/`TD`/`BT` renders it vertically.

use crate::parse::TimelineDiagram;

use super::builder::{SvgBuilder, LABEL_LINE_H};
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const SECTION_H: f64 = 40.0;
const SECTION_GAP: f64 = 10.0;
const PERIOD_H: f64 = 44.0;
const PERIOD_GAP: f64 = 150.0;
/// Horizontal inset of a box inside its period column (both sides).
const BOX_INSET: f64 = 12.0;
/// Gap between the period boxes and the axis, and between the axis and events.
const AXIS_GAP: f64 = 32.0;
const EVENT_BOX_H: f64 = 40.0;
const EVENT_GAP: f64 = 10.0;
/// Approx. chars that fit on one line inside a box at font-size 12.
const EVENT_WRAP: usize = 18;
const BOX_RX: &str = "5";
/// How far each dashed connector continues past the last event before its
/// downward tail arrowhead (upstream ends every connector this way).
const CONNECTOR_TAIL: f64 = 22.0;

mod color;
mod vertical;
use color::darken10;

/// `direction TB`/`TD`/`BT` render the timeline vertically (time top→bottom);
/// `LR`/`RL`/unset keep the default horizontal layout.
fn is_vertical(direction: &Option<String>) -> bool {
    matches!(direction.as_deref(), Some("TB" | "TD" | "BT"))
}

/// Categorical color for a period. Upstream colors by section; a sectionless
/// timeline advances per time-period (`isWithoutSections`) unless
/// `timeline.disableMulticolor` pins it to one flat color.
fn period_color(
    theme: &Theme,
    has_named_section: bool,
    disable_multicolor: bool,
    si: usize,
    idx: usize,
) -> &str {
    let ci = if has_named_section {
        si
    } else if disable_multicolor {
        0
    } else {
        idx
    };
    theme.cscale_color(ci)
}

/// Legible text color for a filled box: white on a dark fill, dark ink otherwise
/// (upstream picks a contrasting label color per section color).
fn text_color_for(fill: &str) -> &'static str {
    if let Some(hex) = fill.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f64;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255) as f64;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255) as f64;
            if 0.299 * r + 0.587 * g + 0.114 * b < 168.0 {
                return "#fff";
            }
        }
    }
    "#333"
}

/// `(max events on any one period, uniform event-box height)`. The tallest
/// wrapped event drives a shared box height so no event overflows, and the line
/// spacing tracks `--font-size` the same way the builder scales tspan `dy`.
fn event_layout(d: &TimelineDiagram, theme: &Theme) -> (usize, f64) {
    let max_events = d
        .sections
        .iter()
        .flat_map(|s| s.periods.iter().map(|p| p.events.len()))
        .max()
        .unwrap_or(0);
    let max_lines = d
        .sections
        .iter()
        .flat_map(|s| s.periods.iter().flat_map(|p| p.events.iter()))
        .map(|ev| wrap_event(ev).len())
        .max()
        .unwrap_or(1)
        .max(1);
    let line_h = LABEL_LINE_H * super::metrics::font_scale(theme.font_size);
    (max_events, EVENT_BOX_H + (max_lines as f64 - 1.0) * line_h)
}

/// Word-wrap an event label to lines of at most `EVENT_WRAP` chars, breaking on
/// spaces. Upstream wraps long events instead of truncating them. A single word
/// longer than the budget is kept intact on its own line rather than hard-split.
fn wrap_event(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= EVENT_WRAP {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut line));
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Draw a filled rounded box with a centered, regular-weight, contrasting label.
#[allow(clippy::too_many_arguments)]
fn box_label(
    svg: &mut SvgBuilder,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: &str,
    class: &str,
    label: &str,
    font_size: u32,
) {
    svg.rect(
        x,
        y,
        w,
        h,
        &format!("class=\"{class}\" fill=\"{color}\" stroke=\"{color}\" stroke-width=\"1\" rx=\"{BOX_RX}\""),
    );
    svg.text(
        x + w / 2.0,
        y + h / 2.0 + 4.0,
        &format!(
            "text-anchor=\"middle\" fill=\"{ink}\" font-size=\"{font_size}\"",
            ink = text_color_for(color)
        ),
        label,
    );
}

pub(crate) fn render(d: &TimelineDiagram, theme: &Theme) -> String {
    if is_vertical(&d.direction) {
        return vertical::render(d, theme);
    }
    let fg = &theme.fg;

    let total_periods: usize = d.sections.iter().map(|s| s.periods.len()).sum();
    let (max_events, event_box_h) = event_layout(d, theme);

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let has_named_section = d.sections.iter().any(|s| s.name.is_some());
    let band_h = if has_named_section {
        SECTION_H + SECTION_GAP
    } else {
        0.0
    };

    let chart_w = (total_periods.max(1) as f64) * PERIOD_GAP;
    let width = PAD * 2.0 + chart_w;
    let events_h = max_events as f64 * (event_box_h + EVENT_GAP);

    let band_y = PAD + title_h;
    let period_y = band_y + band_h;
    let axis_y = period_y + PERIOD_H + AXIS_GAP;
    let events_y0 = axis_y + AXIS_GAP;
    // Every connector drops to one aligned tail below the tallest event stack,
    // ending in a downward arrowhead like upstream.
    let events_bottom = events_y0 + events_h - EVENT_GAP;
    let tail_y = events_bottom + CONNECTOR_TAIL;
    let height = tail_y + PAD;
    let chart_left = PAD;

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    svg.def_arrow_marker("tl-arrow", fg, 7, 6);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    // Section bands: one saturated header per named section spanning its columns.
    let mut x = chart_left;
    for (si, sec) in d.sections.iter().enumerate() {
        let w = sec.periods.len() as f64 * PERIOD_GAP;
        if w > 0.0 {
            if let Some(name) = &sec.name {
                let color = darken10(theme.cscale_color(si));
                box_label(
                    &mut svg,
                    x + 3.0,
                    band_y,
                    w - 6.0,
                    SECTION_H,
                    &color,
                    "tl-section",
                    name,
                    14,
                );
            }
        }
        x += w;
    }

    // Periods above the axis, events below, a dashed connector through both.
    let mut idx = 0usize;
    for (si, sec) in d.sections.iter().enumerate() {
        for period in &sec.periods {
            let base = period_color(theme, has_named_section, d.disable_multicolor, si, idx);
            let color = darken10(base);
            let col_x = chart_left + idx as f64 * PERIOD_GAP;
            let cx = col_x + PERIOD_GAP / 2.0;
            let box_x = col_x + BOX_INSET;
            let box_w = PERIOD_GAP - 2.0 * BOX_INSET;

            // Dark dashed connector from the period, through the axis, down past
            // the events to an aligned tail arrow (upstream tints these dark
            // gray, not with the section color).
            svg.line(
                cx,
                period_y + PERIOD_H,
                cx,
                tail_y,
                &format!(
                    "stroke=\"{fg}\" stroke-width=\"2\" stroke-dasharray=\"5 5\" \
                     marker-end=\"url(#tl-arrow)\""
                ),
            );

            box_label(
                &mut svg,
                box_x,
                period_y,
                box_w,
                PERIOD_H,
                &color,
                "tl-period",
                &period.label,
                13,
            );

            for (ei, ev) in period.events.iter().enumerate() {
                let ey = events_y0 + ei as f64 * (event_box_h + EVENT_GAP);
                box_label(
                    &mut svg,
                    box_x,
                    ey,
                    box_w,
                    event_box_h,
                    &color,
                    "tl-event",
                    &wrap_event(ev).join("\n"),
                    12,
                );
            }
            idx += 1;
        }
    }

    // Thick arrow axis, drawn on top of the dashed connectors it crosses.
    svg.line(
        chart_left,
        axis_y,
        chart_left + chart_w,
        axis_y,
        &format!("stroke=\"{fg}\" stroke-width=\"2.5\" marker-end=\"url(#tl-arrow)\""),
    );

    svg.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{TimelinePeriod, TimelineSection};

    #[test]
    fn produces_svg() {
        let d = TimelineDiagram {
            title: Some("History".into()),
            sections: vec![TimelineSection {
                name: Some("Era".into()),
                periods: vec![TimelinePeriod {
                    label: "2002".into(),
                    events: vec!["LinkedIn".into()],
                }],
            }],
            direction: None,
            disable_multicolor: false,
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">2002<"));
        assert!(svg.contains(">LinkedIn<"));
    }

    #[test]
    fn block_and_arrow_model() {
        // Filled period/event boxes + a thick arrow axis, not dots on a line.
        let d = TimelineDiagram {
            title: None,
            sections: vec![TimelineSection {
                name: Some("Era".into()),
                periods: vec![TimelinePeriod {
                    label: "2002".into(),
                    events: vec!["LinkedIn".into()],
                }],
            }],
            direction: None,
            disable_multicolor: false,
        };
        let svg = render(&d, &Theme::default());
        assert!(!svg.contains("<circle"), "no dot markers");
        assert!(svg.contains("class=\"tl-section\""));
        assert!(svg.contains("class=\"tl-period\""));
        assert!(svg.contains("class=\"tl-event\""));
        assert!(svg.contains("marker-end=\"url(#tl-arrow)\""), "arrow axis");
        assert!(svg.contains("stroke-dasharray"), "dashed connector");
    }

    #[test]
    fn long_event_wraps_instead_of_truncating() {
        let d = TimelineDiagram {
            title: None,
            sections: vec![TimelineSection {
                name: None,
                periods: vec![TimelinePeriod {
                    label: "2004".into(),
                    events: vec!["Decentralized Social Networking".into()],
                }],
            }],
            direction: None,
            disable_multicolor: false,
        };
        let svg = render(&d, &Theme::default());
        // Wrapped across <tspan>s, no ellipsis truncation.
        assert!(!svg.contains('…'));
        assert!(svg.contains("<tspan"));
        assert!(svg.contains(">Decentralized<"));
    }

    fn sectionless(disable_multicolor: bool) -> TimelineDiagram {
        TimelineDiagram {
            title: None,
            sections: vec![TimelineSection {
                name: None,
                periods: vec![
                    TimelinePeriod {
                        label: "2002".into(),
                        events: vec!["LinkedIn".into()],
                    },
                    TimelinePeriod {
                        label: "2004".into(),
                        events: vec!["Facebook".into()],
                    },
                ],
            }],
            direction: None,
            disable_multicolor,
        }
    }

    #[test]
    fn sectionless_timeline_advances_color_per_period() {
        let theme = Theme::default();
        let svg = render(&sectionless(false), &theme);
        // Distinct period colors are present (darkened to the upstream fills).
        assert!(svg.contains(&format!("fill=\"{}\"", darken10(theme.cscale_color(0)))));
        assert!(svg.contains(&format!("fill=\"{}\"", darken10(theme.cscale_color(1)))));
    }

    #[test]
    fn disable_multicolor_keeps_one_color() {
        let theme = Theme::default();
        let svg = render(&sectionless(true), &theme);
        assert!(svg.contains(&format!("fill=\"{}\"", darken10(theme.cscale_color(0)))));
        assert!(!svg.contains(&format!("fill=\"{}\"", darken10(theme.cscale_color(1)))));
    }

    #[test]
    fn connectors_end_in_a_tail_arrow_below_the_events() {
        // Dark dashed connector (not section-tinted) with a downward arrowhead
        // past the last event, darkened fills, white ink, regular weight (#321).
        let svg = render(&two_periods(None), &Theme::default());
        let tails = svg
            .split("<line ")
            .filter(|c| c.contains("dasharray") && c.contains("url(#tl-arrow)"))
            .count();
        assert_eq!(tails, 2, "one tail arrow per period");
        assert!(
            !svg.contains("stroke=\"#FFFFAB\""),
            "connector not fill-tinted"
        );
        assert_eq!(
            text_color_for(&darken10(Theme::default().cscale_color(0))),
            "#fff"
        );
        assert!(!svg.contains("font-weight=\"bold\" font-size=\"13\""));
    }

    /// `(cx, cy)` of each period box (`class="tl-period"` rect) in source order.
    fn period_markers(svg: &str) -> Vec<(f64, f64)> {
        svg.split("<rect ")
            .filter(|c| c.contains("class=\"tl-period\""))
            .filter_map(|c| {
                let (x, y) = (attr(c, "x=\"")?, attr(c, "y=\"")?);
                let (w, h) = (attr(c, "width=\"")?, attr(c, "height=\"")?);
                Some((x + w / 2.0, y + h / 2.0))
            })
            .collect()
    }

    fn attr(chunk: &str, key: &str) -> Option<f64> {
        let start = chunk.find(key)? + key.len();
        let end = chunk[start..].find('"')? + start;
        chunk[start..end].parse().ok()
    }

    fn two_periods(direction: Option<String>) -> TimelineDiagram {
        TimelineDiagram {
            title: None,
            sections: vec![TimelineSection {
                name: None,
                periods: vec![
                    TimelinePeriod {
                        label: "2002".into(),
                        events: vec!["LinkedIn".into()],
                    },
                    TimelinePeriod {
                        label: "2004".into(),
                        events: vec!["Facebook".into()],
                    },
                ],
            }],
            direction,
            disable_multicolor: false,
        }
    }

    #[test]
    fn vertical_direction_stacks_periods_top_down() {
        // `direction TD` runs the axis vertically: period boxes share an x and
        // advance in y (#227).
        let svg = render(&two_periods(Some("TD".into())), &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">2002<") && svg.contains(">LinkedIn<"));
        let m = period_markers(&svg);
        assert_eq!(m.len(), 2);
        assert!((m[0].0 - m[1].0).abs() < 0.01, "boxes share the axis x");
        assert!(m[0].1 < m[1].1, "periods advance downward");
    }

    #[test]
    fn horizontal_direction_keeps_periods_left_to_right() {
        // The default/`LR` layout runs the axis horizontally: boxes share a y
        // and advance in x — the opposite axis from vertical.
        for dir in [None, Some("LR".into())] {
            let svg = render(&two_periods(dir), &Theme::default());
            let m = period_markers(&svg);
            assert_eq!(m.len(), 2);
            assert!((m[0].1 - m[1].1).abs() < 0.01, "boxes share the axis y");
            assert!(m[0].0 < m[1].0, "periods advance rightward");
        }
    }

    #[test]
    fn wrap_event_breaks_on_words() {
        assert_eq!(wrap_event("Facebook"), vec!["Facebook".to_string()]);
        assert_eq!(
            wrap_event("Decentralized Social Networking"),
            vec!["Decentralized".to_string(), "Social Networking".to_string()]
        );
    }

    #[test]
    fn text_color_contrasts_with_fill() {
        assert_eq!(text_color_for("#B9B9FF"), "#333");
        assert_eq!(text_color_for("#444"), "#333"); // 3-digit hex is left as ink
        assert_eq!(text_color_for("#444444"), "#fff");
    }
}
