//! Time-axis tick computation and small axis helpers for the gantt renderer.
//!
//! Picks a legible tick step (capped by label width), aligns the first tick
//! onto an optional named weekday, and formats the `todayMarker` CSS string.

use crate::parse::GanttDiagram;

use super::{AXIS_FONT_SIZE, AXIS_LABEL_CHAR_W, TICK_LABEL_PAD};
use crate::svg::builder::escape;
use crate::svg::gantt_date::{format_date, weekday};
use crate::svg::metrics::text_width;

/// Days per tick for a `tickInterval` value like `1day`, `2week`, `1month`
/// (also the bare `1d`/`1w` units). Returns `None` for an unrecognized unit.
pub(super) fn parse_tick_interval(s: &str) -> Option<f64> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit())?;
    let n: f64 = s[..split].parse().ok()?;
    let mult = match s[split..].trim().to_ascii_lowercase().as_str() {
        "d" | "day" | "days" => 1.0,
        "w" | "week" | "weeks" => 7.0,
        "month" | "months" => 30.0,
        _ => return None,
    };
    Some((n * mult).max(1.0))
}

/// Offset (in days from `start_day`) of the first axis tick so it lands on the
/// `weekday`-named day; `0.0` when no weekday is set or it's unrecognized.
pub(super) fn weekday_tick_offset(weekday_name: Option<&str>, start_day: f64) -> f64 {
    let Some(target) = weekday_name.and_then(weekday_number) else {
        return 0.0;
    };
    let start = start_day.round() as i64;
    (0..7)
        .find(|o| weekday(start + o) == target)
        .map(|o| o as f64)
        .unwrap_or(0.0)
}

fn weekday_number(name: &str) -> Option<i64> {
    match name.trim().to_ascii_lowercase().as_str() {
        "sunday" => Some(0),
        "monday" => Some(1),
        "tuesday" => Some(2),
        "wednesday" => Some(3),
        "thursday" => Some(4),
        "friday" => Some(5),
        "saturday" => Some(6),
        _ => None,
    }
}

/// Turn a `todayMarker` CSS string into an SVG `style` attribute value.
/// Upstream separates the CSS declarations with commas; SVG `style` uses
/// semicolons, so the commas are swapped and the result is attribute-escaped.
pub(super) fn css_style(css: &str) -> String {
    let joined = css
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    escape(&joined)
}

/// Axis ticks as `(offset_px_from_body_x, label)` pairs. `tickInterval` overrides
/// the automatic step; `weekday` aligns the first tick onto that weekday.
pub(super) fn axis_ticks(
    d: &GanttDiagram,
    start_day: f64,
    total_days: f64,
    body_w: f64,
) -> Vec<(f64, String)> {
    // Cap tick density from the widest label: the step must be at least wide
    // enough that a label plus a small gap fits before the next tick, otherwise
    // adjacent labels overlap into an unreadable smear (#244).
    let min_step_days = if body_w > 0.0 {
        (axis_label_width(start_day, total_days, d.axis_format.as_deref()) + TICK_LABEL_PAD)
            * total_days
            / body_w
    } else {
        0.0
    };
    let tick_step = d
        .tick_interval
        .as_deref()
        .and_then(parse_tick_interval)
        .unwrap_or_else(|| pick_tick_step(total_days, min_step_days));
    let mut ticks = Vec::new();
    let mut tick = weekday_tick_offset(d.weekday.as_deref(), start_day);
    while tick <= total_days + 1e-6 {
        ticks.push((
            (tick / total_days) * body_w,
            format_date(start_day + tick, d.axis_format.as_deref()),
        ));
        tick += tick_step;
    }
    ticks
}

/// Estimated pixel width of the widest axis label across the span, sampled at
/// both ends and the middle (month names / weekdays vary in width).
fn axis_label_width(start_day: f64, total_days: f64, axis_format: Option<&str>) -> f64 {
    [0.0, 0.5, 1.0]
        .into_iter()
        .map(|f| {
            let label = format_date(start_day + f * total_days, axis_format);
            text_width(&label, AXIS_LABEL_CHAR_W, AXIS_FONT_SIZE)
        })
        .fold(0.0, f64::max)
}

/// Days per tick: the smallest clean step whose spacing clears `min_step_days`.
fn pick_tick_step(total_days: f64, min_step_days: f64) -> f64 {
    if total_days < 1.0 {
        // Sub-day span: step in clean minute/hour intervals, aiming for at most
        // ~8 ticks so a `HH:mm` axis stays readable.
        const MINUTE: f64 = 1.0 / 1440.0;
        return [
            1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 180.0, 360.0, 720.0,
        ]
        .into_iter()
        .map(|m| m * MINUTE)
        .find(|step| total_days / step <= 8.0)
        .unwrap_or(720.0 * MINUTE);
    }
    // Calendar-friendly ladder (days). Picking the smallest rung that clears the
    // label-width floor keeps ticks as dense as legibility allows and widens the
    // step automatically as the range grows.
    const LADDER: [f64; 11] = [
        1.0, 2.0, 3.0, 5.0, 7.0, 14.0, 30.0, 60.0, 90.0, 180.0, 365.0,
    ];
    LADDER
        .into_iter()
        .find(|step| *step >= min_step_days)
        .unwrap_or(365.0)
}
