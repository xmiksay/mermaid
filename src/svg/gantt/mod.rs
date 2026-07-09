//! Gantt chart renderer.
//!
//! Computes per-task absolute start (in "days from project start") by resolving
//! `after <id>` and `AfterPrevious` references, then lays out one bar per task
//! in vertical sequence with a time axis at the bottom (top with `topAxis`).

mod axis;
mod tasks;
#[cfg(test)]
mod tests;

use crate::parse::GanttDiagram;

use super::builder::{fnum, SvgBuilder};
use super::gantt_date::{today_days, Excludes};
use super::interact::{close_click, open_click};
use super::metrics::text_width;
use super::theme::Theme;

use axis::{axis_ticks, css_style};
use tasks::{chart_span, colors_for, inside_text_ink, resolve_tasks, Resolved};

const BAR_H: f64 = 20.0;
const ROW_GAP: f64 = 12.0;
const ROW_H: f64 = BAR_H + ROW_GAP;
const HEADER_H: f64 = 56.0;
const AXIS_H: f64 = 26.0;
const PAD: f64 = 16.0;
const TIME_COL_MIN_W: f64 = 480.0;
/// Narrow left gutter reserved for section names when a chart has any; the
/// gutter widens to fit the longest section label. Upstream keeps task names in
/// the chart (inside/next to their bars), so only section names live here.
const LABEL_GUTTER_MIN: f64 = 75.0;

/// Task label font; sampled at [`AXIS_LABEL_CHAR_W`] to decide whether a name
/// fits inside its bar (upstream draws it inside when it fits, else just right).
const TASK_FONT_SIZE: f64 = 11.0;
/// Section-title font (left gutter).
const SECTION_FONT_SIZE: f64 = 13.0;

/// Full-width section background bands, cycling through four styles like
/// upstream's `section0..3`. The default theme resolves to a pale lavender /
/// blank / pale-yellow / blank pattern; the `(fill, fill-opacity)` pairs bake
/// in the `.section { opacity: 0.2 }` rule so a plain `rect` reproduces it.
const SECTION_BANDS: [(&str, &str); 4] = [
    ("#6666ff", "0.098"), // sectionBkgColor rgba(102,102,255,0.49) × 0.2
    ("#ffffff", "0.2"),   // altSectionBkgColor white × 0.2
    ("#fff400", "0.2"),   // sectionBkgColor2 × 0.2
    ("#ffffff", "0.2"),   // altSectionBkgColor white × 0.2
];

/// Axis tick labels are drawn at this font size; the per-glyph width below is
/// tuned so an ISO date (`2026-01-01`) estimates to ~59px, matching what the
/// browser renders — the basis for capping tick density so labels never smear.
const AXIS_FONT_SIZE: f64 = 11.0;
const AXIS_LABEL_CHAR_W: f64 = 7.5;
/// Minimum clear gap (px) kept between one label's end and the next tick.
const TICK_LABEL_PAD: f64 = 4.0;

pub(crate) fn render(d: &GanttDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    // Step 1 — resolve absolute start positions in "days from epoch".
    let resolved = resolve_tasks(d);
    let (start_day, total_days, sub_day) = chart_span(&resolved);
    let min_bar_dur = if sub_day { 0.0 } else { 0.5 };

    // Step 2 — compute dimensions. One row per task (task names render in the
    // chart, not in a left column); section names share the left gutter,
    // vertically centered across their tasks. `vert` markers span the chart
    // rather than occupying a row, so they're excluded from row allocation.
    let n_rows: usize = d
        .sections
        .iter()
        .map(|s| s.tasks.iter().filter(|t| !t.vert).count())
        .sum();
    let body_h = n_rows as f64 * ROW_H;
    let label_gutter = section_gutter(d);
    let time_col_w = TIME_COL_MIN_W;
    let body_x = PAD + label_gutter;
    let body_w = time_col_w;

    // Axis ticks are needed both to size the canvas (the final tick label
    // overhangs the chart's right edge) and to draw the axis below.
    let ticks = axis_ticks(d, start_day, total_days, body_w);

    // Upstream grows the canvas so the rightmost bar, its outside-placed label
    // and the final axis tick label all fit — the chart body alone is not
    // enough for a short bar labelled to its right near the chart end (#311).
    // Keep at least the old minimum width so unaffected charts don't shift.
    let content_right = content_right_extent(
        d,
        &resolved,
        start_day,
        total_days,
        body_x,
        body_w,
        min_bar_dur,
        &ticks,
    );
    let width = (content_right + PAD).max(label_gutter + time_col_w + PAD * 2.0);
    let height = HEADER_H + AXIS_H + body_h + PAD * 2.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    // Title
    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 20.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }
    // Upstream's default layout draws the axis at the bottom; `topAxis` moves it
    // to the top. `body_top` is where the task rows begin, `axis_y` the top of
    // the axis band (below the rows in the default layout).
    let (body_top, axis_y) = if d.top_axis {
        (HEADER_H + AXIS_H, HEADER_H)
    } else {
        (HEADER_H, HEADER_H + body_h)
    };
    let body_bottom = body_top + body_h;

    // Axis baseline sits on the edge the rows share with the axis band.
    let axis_line_y = if d.top_axis {
        axis_y + AXIS_H - 1.0
    } else {
        axis_y
    };
    svg.line(
        body_x,
        axis_line_y,
        body_x + body_w,
        axis_line_y,
        "stroke=\"#999\" stroke-width=\"1\"",
    );
    for (dx, label) in &ticks {
        let x = body_x + dx;
        svg.text(
            x + 2.0,
            axis_y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"11\""),
            label,
        );
    }

    // Full-width section background bands behind the rows (upstream draws one
    // colored band per section, cycling four styles). Bands span the gutter and
    // the chart so section names sit over their own band.
    let band_x = PAD;
    let band_w = label_gutter + body_w;
    {
        let mut band_y = body_top;
        for (i, section) in d.sections.iter().enumerate() {
            let h = section.tasks.iter().filter(|t| !t.vert).count() as f64 * ROW_H;
            if h > 0.0 {
                let (fill, opacity) = SECTION_BANDS[i % SECTION_BANDS.len()];
                svg.rect(
                    band_x,
                    band_y,
                    band_w,
                    h,
                    &format!("fill=\"{fill}\" fill-opacity=\"{opacity}\" stroke=\"none\""),
                );
            }
            band_y += h;
        }
    }

    // Excluded-day shading (weekends etc.): a light band per non-working day
    // behind the bars, matching upstream's `exclude-range` rects.
    let excludes = Excludes::parse(&d.excludes, d.date_format.as_deref(), d.weekend.as_deref());
    if excludes.active() {
        let day_w = body_w / total_days;
        let first = start_day.floor() as i64;
        let last = (start_day + total_days).ceil() as i64;
        for day in first..last {
            if excludes.is_excluded(day) {
                let x = body_x + ((day as f64 - start_day) / total_days) * body_w;
                svg.rect(
                    x,
                    body_top,
                    day_w,
                    body_h,
                    "fill=\"#000\" fill-opacity=\"0.04\"",
                );
            }
        }
    }

    // Full-height vertical grid lines at every axis tick, spanning the chart
    // body. Upstream's d3 axis draws these light-grey ticks through the rows
    // (over the section bands, behind the bars); we match with `gridColor`
    // (`lightgrey`) at the same 0.8 opacity.
    for (dx, _) in &ticks {
        let x = body_x + dx;
        svg.line(
            x,
            body_top,
            x,
            body_bottom,
            "stroke=\"#d3d3d3\" stroke-width=\"1\" opacity=\"0.8\"",
        );
    }

    // Today marker: positioned at the *current* date (system clock), drawn only
    // when it falls inside the chart's range. `todayMarker off` suppresses it;
    // any other value is a CSS style applied to the marker line; the default is
    // a red dashed line (upstream always draws a marker at today).
    let today_style = d.today_marker.as_deref();
    if today_style != Some("off") {
        let rel = today_days() as f64 - start_day;
        if rel >= 0.0 && rel <= total_days {
            let x = body_x + (rel / total_days) * body_w;
            let attrs = match today_style {
                Some(css) => format!("style=\"{}\"", css_style(css)),
                None => "stroke=\"#d33\" stroke-width=\"2\" stroke-dasharray=\"4 3\"".to_string(),
            };
            svg.line(x, body_top, x, body_bottom, &attrs);
            svg.text(
                x + 4.0,
                body_top + 12.0,
                "fill=\"#d33\" font-size=\"11\" font-weight=\"bold\"",
                "today",
            );
        }
    }

    // Body — one row per task, task labels drawn in the chart. Section names go
    // in the left gutter, vertically centered across the section's rows.
    let mut y = body_top;
    let mut flat_idx = 0;
    for section in &d.sections {
        let section_top = y;
        for task in &section.tasks {
            let r = &resolved[flat_idx];
            let x = body_x + ((r.start_day - start_day) / total_days) * body_w;
            if task.vert {
                // Vertical marker: a thick navy full-height line at the start
                // date with the bold navy label centered below the axis
                // (upstream `vert` styling). Excluded from row allocation — it
                // consumes no row height and does not advance `y`. Duration is
                // ignored.
                svg.line(
                    x,
                    body_top,
                    x,
                    body_bottom,
                    "stroke=\"#000080\" stroke-width=\"4\"",
                );
                // Label below the chart: under the bottom axis band by default,
                // under the last row when the axis is on top.
                let label_y = if d.top_axis {
                    body_bottom + 12.0
                } else {
                    axis_y + AXIS_H + 12.0
                };
                svg.text(
                    x,
                    label_y,
                    "text-anchor=\"middle\" fill=\"#000080\" font-size=\"11\" font-weight=\"bold\"",
                    &task.name,
                );
                flat_idx += 1;
                continue;
            }
            let cy = y + BAR_H / 2.0;
            let (fill, stroke) = colors_for(task.status, task.crit);
            let sw = if task.crit { 2 } else { 1 };
            if let Some(click) = &task.click {
                open_click(&mut svg, click);
            }
            if task.milestone {
                // Diamond centered on the start date; duration is ignored, label
                // to the right of the diamond.
                let rad = (BAR_H - 4.0) / 2.0;
                svg.path(
                    &format!(
                        "M {} {} L {} {} L {} {} L {} {} Z",
                        fnum(x),
                        fnum(cy - rad),
                        fnum(x + rad),
                        fnum(cy),
                        fnum(x),
                        fnum(cy + rad),
                        fnum(x - rad),
                        fnum(cy),
                    ),
                    &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\""),
                );
                svg.text(
                    x + rad + 4.0,
                    cy + 4.0,
                    &format!("fill=\"{fg}\" font-size=\"11\" font-style=\"italic\""),
                    &task.name,
                );
            } else {
                // Bar with its label inside when it fits, otherwise just right
                // of the bar (upstream behavior).
                let w = ((r.duration.max(min_bar_dur) / total_days) * body_w).max(2.0);
                svg.rect(
                    x,
                    y + 2.0,
                    w,
                    BAR_H - 4.0,
                    &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\" rx=\"3\""),
                );
                let label_w = text_width(&task.name, AXIS_LABEL_CHAR_W, TASK_FONT_SIZE);
                if label_w + 8.0 <= w {
                    // Inside the bar: white ink on normal/crit, dark on
                    // active/done (upstream `.taskText` vs `.activeText`/
                    // `.doneText`).
                    let ink = inside_text_ink(task.status);
                    svg.text(
                        x + w / 2.0,
                        cy + 4.0,
                        &format!(
                            "text-anchor=\"middle\" fill=\"{ink}\" font-size=\"{TASK_FONT_SIZE}\""
                        ),
                        &task.name,
                    );
                } else {
                    svg.text(
                        x + w + 4.0,
                        cy + 4.0,
                        &format!("fill=\"{fg}\" font-size=\"{TASK_FONT_SIZE}\""),
                        &task.name,
                    );
                }
            }
            if let Some(click) = &task.click {
                close_click(&mut svg, click);
            }
            y += ROW_H;
            flat_idx += 1;
        }
        if !section.name.is_empty() && y > section_top {
            // Section title centered vertically over its band, left-aligned in
            // the gutter.
            svg.text(
                PAD,
                (section_top + y) / 2.0 + 4.0,
                &format!("fill=\"{fg}\" font-size=\"{SECTION_FONT_SIZE}\""),
                &section.name,
            );
        }
    }

    svg.finish()
}

/// Rightmost pixel reached by content that can overhang the chart body: a bar's
/// (or milestone's) label placed to the *right* of a short bar near the chart
/// end, a `vert` marker's centered label, and the final axis tick label (drawn
/// just past the last tick). Mirrors the label placement in `render` so the
/// caller can grow the canvas to fit them (#311). Never returns less than the
/// body's right edge.
#[allow(clippy::too_many_arguments)]
fn content_right_extent(
    d: &GanttDiagram,
    resolved: &[Resolved],
    start_day: f64,
    total_days: f64,
    body_x: f64,
    body_w: f64,
    min_bar_dur: f64,
    ticks: &[(f64, String)],
) -> f64 {
    let mut max_right = body_x + body_w;
    let mut flat_idx = 0;
    for section in &d.sections {
        for task in &section.tasks {
            let r = &resolved[flat_idx];
            flat_idx += 1;
            let x = body_x + ((r.start_day - start_day) / total_days) * body_w;
            let label_w = text_width(&task.name, AXIS_LABEL_CHAR_W, TASK_FONT_SIZE);
            if task.vert {
                // Centered label below the marker overhangs by half its width.
                max_right = max_right.max(x + label_w / 2.0);
            } else if task.milestone {
                let rad = (BAR_H - 4.0) / 2.0;
                max_right = max_right.max(x + rad + 4.0 + label_w);
            } else {
                let w = ((r.duration.max(min_bar_dur) / total_days) * body_w).max(2.0);
                // Label sits right of the bar only when it does not fit inside.
                if label_w + 8.0 > w {
                    max_right = max_right.max(x + w + 4.0 + label_w);
                }
            }
        }
    }
    // The final axis tick label is drawn at `tick x + 2.0`.
    if let Some((dx, label)) = ticks.last() {
        let x = body_x + dx;
        max_right = max_right.max(x + 2.0 + text_width(label, AXIS_LABEL_CHAR_W, AXIS_FONT_SIZE));
    }
    max_right
}

/// Width of the left gutter reserved for section names: enough for the widest
/// section label plus padding, or `0` when no section is named. Task names no
/// longer live here, so the gutter stays narrow.
fn section_gutter(d: &GanttDiagram) -> f64 {
    let widest = d
        .sections
        .iter()
        .filter(|s| !s.name.is_empty())
        .map(|s| text_width(&s.name, AXIS_LABEL_CHAR_W, SECTION_FONT_SIZE))
        .fold(0.0_f64, f64::max);
    if widest <= 0.0 {
        0.0
    } else {
        (widest + PAD).max(LABEL_GUTTER_MIN)
    }
}
