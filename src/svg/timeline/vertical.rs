//! Vertical timeline (`direction TB`/`TD`/`BT`): the horizontal block-and-arrow
//! model rotated a quarter turn. The arrow axis runs down the middle, period
//! boxes sit to its left, events flow rightward, a dashed connector links each
//! period across the axis to its events, and sections become bands down the left
//! margin.

use crate::parse::TimelineDiagram;

use super::super::builder::SvgBuilder;
use super::super::theme::Theme;
use super::color::darken10;
use super::{
    box_label, event_layout, period_color, text_color_for, wrap_event, AXIS_GAP, BOX_RX,
    CONNECTOR_TAIL, EVENT_BOX_H, EVENT_GAP, PAD, PERIOD_H, TITLE_GAP,
};

/// Width of the far-left section band, the period boxes, and each event box.
const V_BAND_W: f64 = 40.0;
const V_PERIOD_W: f64 = 110.0;
const V_EVENT_W: f64 = 120.0;
const V_GAP: f64 = 10.0;

pub(super) fn render(d: &TimelineDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;

    let total_periods: usize = d.sections.iter().map(|s| s.periods.len()).sum();
    let (max_events, event_box_h) = event_layout(d, theme);
    let row_h = EVENT_BOX_H.max(event_box_h).max(PERIOD_H) + EVENT_GAP * 2.0;

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let has_named_section = d.sections.iter().any(|s| s.name.is_some());
    let band_w = if has_named_section {
        V_BAND_W + V_GAP
    } else {
        0.0
    };

    let period_x = PAD + band_w;
    let axis_x = period_x + V_PERIOD_W + AXIS_GAP;
    let events_x0 = axis_x + AXIS_GAP;
    let events_w = max_events as f64 * (V_EVENT_W + EVENT_GAP);
    // Every connector runs to one aligned tail right of the widest event row,
    // ending in a rightward arrowhead like upstream.
    let events_right = events_x0 + events_w - EVENT_GAP;
    let tail_x = events_right + CONNECTOR_TAIL;

    let chart_h = (total_periods.max(1) as f64) * row_h;
    let width = tail_x + PAD;
    let height = PAD * 2.0 + title_h + chart_h;

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

    let chart_top = PAD + title_h;

    // Section bands: rotated headers down the left margin.
    let mut y = chart_top;
    for (si, sec) in d.sections.iter().enumerate() {
        let h = sec.periods.len() as f64 * row_h;
        if h > 0.0 {
            if let Some(name) = &sec.name {
                let color = darken10(theme.cscale_color(si));
                svg.rect(
                    PAD,
                    y + 3.0,
                    V_BAND_W,
                    h - 6.0,
                    &format!("class=\"tl-section\" fill=\"{color}\" stroke=\"{color}\" stroke-width=\"1\" rx=\"{BOX_RX}\""),
                );
                let bx = PAD + V_BAND_W / 2.0;
                let by = y + h / 2.0;
                svg.text(
                    bx,
                    by,
                    &format!(
                        "text-anchor=\"middle\" transform=\"rotate(-90 {bx} {by})\" \
                         fill=\"{ink}\" font-size=\"14\"",
                        ink = text_color_for(&color),
                        bx = super::super::builder::fnum(bx),
                        by = super::super::builder::fnum(by),
                    ),
                    name,
                );
            }
        }
        y += h;
    }

    // Periods left of the axis, events right, dashed connector across.
    let mut idx = 0usize;
    for (si, sec) in d.sections.iter().enumerate() {
        for period in &sec.periods {
            let base = period_color(theme, has_named_section, d.disable_multicolor, si, idx);
            let color = darken10(base);
            let cy = chart_top + idx as f64 * row_h + row_h / 2.0;

            // Dark dashed connector across the axis to an aligned tail arrow
            // right of the events (upstream tints these dark gray, not the fill).
            svg.line(
                period_x + V_PERIOD_W,
                cy,
                tail_x,
                cy,
                &format!(
                    "stroke=\"{fg}\" stroke-width=\"2\" stroke-dasharray=\"5 5\" \
                     marker-end=\"url(#tl-arrow)\""
                ),
            );

            box_label(
                &mut svg,
                period_x,
                cy - PERIOD_H / 2.0,
                V_PERIOD_W,
                PERIOD_H,
                &color,
                "tl-period",
                &period.label,
                13,
            );

            for (ei, ev) in period.events.iter().enumerate() {
                let ex = events_x0 + ei as f64 * (V_EVENT_W + EVENT_GAP);
                box_label(
                    &mut svg,
                    ex,
                    cy - event_box_h / 2.0,
                    V_EVENT_W,
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

    // Thick arrow axis down the middle, on top of the connectors it crosses.
    svg.line(
        axis_x,
        chart_top,
        axis_x,
        chart_top + chart_h,
        &format!("stroke=\"{fg}\" stroke-width=\"2.5\" marker-end=\"url(#tl-arrow)\""),
    );

    svg.finish()
}
