//! Timeline renderer. Horizontal axis, periods spaced evenly, events stacked
//! above each period. Sections appear as colored bands.

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
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;

    let total_periods: usize = d.sections.iter().map(|s| s.periods.len()).sum();
    let max_events: usize = d
        .sections
        .iter()
        .flat_map(|s| s.periods.iter().map(|p| p.events.len()))
        .max()
        .unwrap_or(0);

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let has_named_section = d.sections.iter().any(|s| s.name.is_some());
    let band_h = if has_named_section { SECTION_BAND } else { 0.0 };

    // Tallest event drives a uniform box height so wrapped events don't overflow.
    let max_lines = d
        .sections
        .iter()
        .flat_map(|s| s.periods.iter().flat_map(|p| p.events.iter()))
        .map(|ev| wrap_event(ev).len())
        .max()
        .unwrap_or(1)
        .max(1);
    let event_box_h = EVENT_BOX_H + (max_lines as f64 - 1.0) * LABEL_LINE_H;

    let chart_w = (total_periods.max(1) as f64) * PERIOD_GAP;
    let width = PAD * 2.0 + chart_w;
    let events_h = max_events as f64 * (event_box_h + EVENT_GAP) + EVENT_GAP;
    let height = PAD * 2.0 + title_h + band_h + AXIS_Y_OFFSET + events_h + 30.0;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

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
            let color = theme.pie_color(si);
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

    // Periods: tick, label, events.
    let mut idx = 0usize;
    for (si, sec) in d.sections.iter().enumerate() {
        let color = theme.pie_color(si);
        for period in &sec.periods {
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
        };
        let svg = render(&d, &Theme::default());
        // Wrapped across <tspan>s, no ellipsis truncation.
        assert!(!svg.contains('…'));
        assert!(svg.contains("<tspan"));
        assert!(svg.contains(">Decentralized<"));
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
