//! Timeline renderer. Horizontal axis, periods spaced evenly, events stacked
//! above each period. Sections appear as colored bands.

use crate::parse::TimelineDiagram;

use super::builder::{escape, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const SECTION_BAND: f64 = 26.0;
const PERIOD_GAP: f64 = 140.0;
const AXIS_Y_OFFSET: f64 = 60.0;
const EVENT_BOX_H: f64 = 36.0;
const EVENT_GAP: f64 = 8.0;
const EVENT_BOX_W: f64 = 120.0;

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

    let chart_w = (total_periods.max(1) as f64) * PERIOD_GAP;
    let width = PAD * 2.0 + chart_w;
    let events_h = max_events as f64 * (EVENT_BOX_H + EVENT_GAP) + EVENT_GAP;
    let height = PAD * 2.0 + title_h + band_h + AXIS_Y_OFFSET + events_h + 30.0;

    let mut svg = SvgBuilder::new(width, height);

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
                let ey = axis_y + 36.0 + ei as f64 * (EVENT_BOX_H + EVENT_GAP);
                let ex = cx - EVENT_BOX_W / 2.0;
                svg.rect(
                    ex,
                    ey,
                    EVENT_BOX_W,
                    EVENT_BOX_H,
                    &format!("fill=\"{color}\" fill-opacity=\"0.15\" stroke=\"{color}\" stroke-width=\"1\" rx=\"4\""),
                );
                let max_chars = 18;
                let display: String = if ev.chars().count() > max_chars {
                    let mut s: String = ev.chars().take(max_chars - 1).collect();
                    s.push('…');
                    s
                } else {
                    ev.clone()
                };
                let _ = escape;
                svg.text(
                    cx,
                    ey + EVENT_BOX_H / 2.0 + 4.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                    &display,
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
}
