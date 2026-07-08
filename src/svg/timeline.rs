//! Timeline renderer. Periods are spaced evenly along an axis with their events
//! stacked beside them and sections drawn as colored bands. The axis is
//! horizontal by default; `direction TB`/`TD`/`BT` renders it vertically.

use crate::parse::TimelineDiagram;

use super::builder::{SvgBuilder, LABEL_LINE_H};
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const SECTION_BAND: f64 = 26.0;
const PERIOD_GAP: f64 = 140.0;
const AXIS_Y_OFFSET: f64 = 60.0;
const EVENT_BOX_H: f64 = 36.0;
const EVENT_GAP: f64 = 8.0;
const EVENT_BOX_W: f64 = 120.0;
/// Approx. chars that fit on one line inside an event box at font-size 12.
const EVENT_WRAP: usize = 18;
/// Vertical layout: gap left of the axis (mirrors `AXIS_Y_OFFSET`) and the width
/// reserved for the period label sitting to the axis's right, before events.
const AXIS_LEFT_GAP: f64 = 20.0;
const PERIOD_LABEL_W: f64 = 70.0;

/// `direction TB`/`TD`/`BT` render the timeline vertically (time top→bottom);
/// `LR`/`RL`/unset keep the default horizontal layout.
fn is_vertical(direction: &Option<String>) -> bool {
    matches!(direction.as_deref(), Some("TB" | "TD" | "BT"))
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

pub(crate) fn render(d: &TimelineDiagram, theme: &Theme) -> String {
    if is_vertical(&d.direction) {
        return render_vertical(d, theme);
    }
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    let total_periods: usize = d.sections.iter().map(|s| s.periods.len()).sum();
    let (max_events, event_box_h) = event_layout(d, theme);

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let has_named_section = d.sections.iter().any(|s| s.name.is_some());
    let band_h = if has_named_section { SECTION_BAND } else { 0.0 };

    let chart_w = (total_periods.max(1) as f64) * PERIOD_GAP;
    let width = PAD * 2.0 + chart_w;
    let events_h = max_events as f64 * (event_box_h + EVENT_GAP) + EVENT_GAP;
    let height = PAD * 2.0 + title_h + band_h + AXIS_Y_OFFSET + events_h + 30.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let band_y = PAD + title_h;
    let axis_y = band_y + band_h + AXIS_Y_OFFSET;
    let chart_left = PAD;

    // Section bands.
    let mut x = chart_left;
    for (si, sec) in d.sections.iter().enumerate() {
        let w = sec.periods.len() as f64 * PERIOD_GAP;
        if w > 0.0 && sec.name.is_some() {
            let color = theme.cscale_color(si);
            svg.rect(
                x,
                band_y,
                w,
                SECTION_BAND - 4.0,
                &format!(
                    "fill=\"{color}\" fill-opacity=\"0.25\" stroke=\"{color}\" stroke-width=\"1\""
                ),
            );
            if let Some(name) = &sec.name {
                svg.text(
                    x + w / 2.0,
                    band_y + SECTION_BAND / 2.0 + 2.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
                    name,
                );
            }
        }
        x += w;
    }

    // Axis line.
    svg.line(
        chart_left,
        axis_y,
        chart_left + chart_w,
        axis_y,
        &format!("stroke=\"{fg_muted}\" stroke-width=\"2\""),
    );

    // Periods: tick, label, events. Upstream colors by section, but a
    // sectionless timeline advances the color per time-period instead
    // (unless `timeline.disableMulticolor` is set, keeping it one flat color).
    let mut idx = 0usize;
    for (si, sec) in d.sections.iter().enumerate() {
        for period in &sec.periods {
            let color_idx = if has_named_section {
                si
            } else if d.disable_multicolor {
                0
            } else {
                idx
            };
            let color = theme.cscale_color(color_idx);
            let cx = chart_left + idx as f64 * PERIOD_GAP + PERIOD_GAP / 2.0;
            svg.circle(
                cx,
                axis_y,
                6.0,
                &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
            );
            svg.text(
                cx,
                axis_y + 22.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""
                ),
                &period.label,
            );

            for (ei, ev) in period.events.iter().enumerate() {
                let ey = axis_y + 36.0 + ei as f64 * (event_box_h + EVENT_GAP);
                let ex = cx - EVENT_BOX_W / 2.0;
                svg.rect(
                    ex,
                    ey,
                    EVENT_BOX_W,
                    event_box_h,
                    &format!("fill=\"{color}\" fill-opacity=\"0.15\" stroke=\"{color}\" stroke-width=\"1\" rx=\"4\""),
                );
                svg.text(
                    cx,
                    ey + event_box_h / 2.0 + 4.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                    &wrap_event(ev).join("\n"),
                );
            }
            idx += 1;
        }
    }

    svg.finish()
}

/// Vertical timeline (`direction TB`/`TD`/`BT`): the axis runs down the left,
/// periods stack top→bottom with their labels to the right of the axis, and each
/// period's events flow rightward in a row. Sections become colored bands down
/// the left margin. This is the horizontal layout rotated a quarter turn.
fn render_vertical(d: &TimelineDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    let total_periods: usize = d.sections.iter().map(|s| s.periods.len()).sum();
    let (max_events, event_box_h) = event_layout(d, theme);

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let has_named_section = d.sections.iter().any(|s| s.name.is_some());
    let band_w = if has_named_section { SECTION_BAND } else { 0.0 };

    let axis_x = PAD + band_w + AXIS_LEFT_GAP;
    let events_x0 = axis_x + PERIOD_LABEL_W;
    let events_w = max_events as f64 * (EVENT_BOX_W + EVENT_GAP) + EVENT_GAP;

    let chart_h = (total_periods.max(1) as f64) * PERIOD_GAP;
    let width = events_x0 + events_w + PAD;
    let height = PAD * 2.0 + title_h + chart_h;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let chart_top = PAD + title_h;

    // Section bands (vertical strips down the left margin).
    let mut y = chart_top;
    for (si, sec) in d.sections.iter().enumerate() {
        let h = sec.periods.len() as f64 * PERIOD_GAP;
        if h > 0.0 && sec.name.is_some() {
            let color = theme.cscale_color(si);
            svg.rect(
                PAD,
                y,
                SECTION_BAND - 4.0,
                h,
                &format!(
                    "fill=\"{color}\" fill-opacity=\"0.25\" stroke=\"{color}\" stroke-width=\"1\""
                ),
            );
            if let Some(name) = &sec.name {
                let bx = PAD + (SECTION_BAND - 4.0) / 2.0;
                let by = y + h / 2.0;
                svg.text(
                    bx,
                    by,
                    &format!(
                        "text-anchor=\"middle\" transform=\"rotate(-90 {bx} {by})\" \
                         fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\"",
                        bx = super::builder::fnum(bx),
                        by = super::builder::fnum(by),
                    ),
                    name,
                );
            }
        }
        y += h;
    }

    // Axis line (vertical).
    svg.line(
        axis_x,
        chart_top,
        axis_x,
        chart_top + chart_h,
        &format!("stroke=\"{fg_muted}\" stroke-width=\"2\""),
    );

    // Periods: marker on the axis, label to its right, events flowing rightward.
    let mut idx = 0usize;
    for (si, sec) in d.sections.iter().enumerate() {
        for period in &sec.periods {
            let color_idx = if has_named_section {
                si
            } else if d.disable_multicolor {
                0
            } else {
                idx
            };
            let color = theme.cscale_color(color_idx);
            let cy = chart_top + idx as f64 * PERIOD_GAP + PERIOD_GAP / 2.0;
            svg.circle(
                axis_x,
                cy,
                6.0,
                &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
            );
            svg.text(
                axis_x + 14.0,
                cy + 4.0,
                &format!(
                    "text-anchor=\"start\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""
                ),
                &period.label,
            );

            for (ei, ev) in period.events.iter().enumerate() {
                let ex = events_x0 + ei as f64 * (EVENT_BOX_W + EVENT_GAP);
                let ey = cy - event_box_h / 2.0;
                svg.rect(
                    ex,
                    ey,
                    EVENT_BOX_W,
                    event_box_h,
                    &format!("fill=\"{color}\" fill-opacity=\"0.15\" stroke=\"{color}\" stroke-width=\"1\" rx=\"4\""),
                );
                svg.text(
                    ex + EVENT_BOX_W / 2.0,
                    ey + event_box_h / 2.0 + 4.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                    &wrap_event(ev).join("\n"),
                );
            }
            idx += 1;
        }
    }

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
        // Distinct period colors are present.
        assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(0))));
        assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(1))));
    }

    #[test]
    fn disable_multicolor_keeps_one_color() {
        let theme = Theme::default();
        let svg = render(&sectionless(true), &theme);
        assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(0))));
        assert!(!svg.contains(&format!("fill=\"{}\"", theme.cscale_color(1))));
    }

    /// `(cx, cy)` of each period marker (`r="6"` circle) in source order.
    fn period_markers(svg: &str) -> Vec<(f64, f64)> {
        svg.split("<circle ")
            .filter(|c| c.contains("r=\"6\""))
            .filter_map(|c| Some((attr(c, "cx=\"")?, attr(c, "cy=\"")?)))
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
        // `direction TD` runs the axis vertically: period markers share an x and
        // advance in y (#227).
        let svg = render(&two_periods(Some("TD".into())), &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">2002<") && svg.contains(">LinkedIn<"));
        let m = period_markers(&svg);
        assert_eq!(m.len(), 2);
        assert!((m[0].0 - m[1].0).abs() < 0.01, "markers share the axis x");
        assert!(m[0].1 < m[1].1, "periods advance downward");
    }

    #[test]
    fn horizontal_direction_keeps_periods_left_to_right() {
        // The default/`LR` layout runs the axis horizontally: markers share a y
        // and advance in x — the opposite axis from vertical.
        for dir in [None, Some("LR".into())] {
            let svg = render(&two_periods(dir), &Theme::default());
            let m = period_markers(&svg);
            assert_eq!(m.len(), 2);
            assert!((m[0].1 - m[1].1).abs() < 0.01, "markers share the axis y");
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
}
