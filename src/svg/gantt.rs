//! Gantt chart renderer.
//!
//! Computes per-task absolute start (in "days from project start") by resolving
//! `after <id>` and `AfterPrevious` references, then lays out one bar per task
//! in vertical sequence with a time axis at the top.

use std::collections::HashMap;

use crate::parse::{GanttDiagram, TaskEnd, TaskStart, TaskStatus};

use super::builder::{escape, fnum, SvgBuilder};
use super::gantt_date::{format_date, parse_datetime, today_days, weekday, Excludes};
use super::interact::{close_click, open_click};
use super::theme::Theme;

const LABEL_COL_W: f64 = 200.0;
const BAR_H: f64 = 20.0;
const ROW_GAP: f64 = 12.0;
const HEADER_H: f64 = 56.0;
const AXIS_H: f64 = 26.0;
const SECTION_H: f64 = 24.0;
const PAD: f64 = 16.0;
const TIME_COL_MIN_W: f64 = 480.0;

pub(crate) fn render(d: &GanttDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    // Step 1 — resolve absolute start positions in "days from epoch".
    let resolved = resolve_tasks(d);
    let project_start = resolved
        .iter()
        .map(|t| t.start_day)
        .fold(f64::INFINITY, f64::min);
    let project_end = resolved
        .iter()
        .map(|t| t.start_day + t.duration)
        .fold(f64::NEG_INFINITY, f64::max);

    let (start_day, total_days) = if project_start.is_finite() && project_end.is_finite() {
        (project_start, (project_end - project_start).max(1.0))
    } else {
        (0.0, 1.0)
    };

    // Step 2 — compute dimensions.
    let n_rows: usize = d
        .sections
        .iter()
        .map(|s| s.tasks.len() + if s.name.is_empty() { 0 } else { 1 })
        .sum();
    let body_h = n_rows as f64 * (BAR_H + ROW_GAP);
    let time_col_w = TIME_COL_MIN_W;
    let width = LABEL_COL_W + time_col_w + PAD * 2.0;
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

    let body_x = PAD + LABEL_COL_W;
    let body_w = time_col_w;
    let axis_y = HEADER_H;

    // Axis line + day ticks
    svg.line(
        body_x,
        axis_y + AXIS_H - 1.0,
        body_x + body_w,
        axis_y + AXIS_H - 1.0,
        "stroke=\"#999\" stroke-width=\"1\"",
    );
    // `tickInterval Nday|Nweek|Nmonth` overrides the automatic step; a
    // `weekday` sets which weekday the first tick lands on (week alignment).
    let tick_step = d
        .tick_interval
        .as_deref()
        .and_then(parse_tick_interval)
        .unwrap_or_else(|| pick_tick_step(total_days));
    let mut tick = weekday_tick_offset(d.weekday.as_deref(), start_day);
    while tick <= total_days + 1e-6 {
        let x = body_x + (tick / total_days) * body_w;
        svg.line(
            x,
            axis_y,
            x,
            axis_y + AXIS_H,
            "stroke=\"#bbb\" stroke-width=\"1\"",
        );
        svg.text(
            x + 2.0,
            axis_y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"11\""),
            &format_date((start_day + tick).round() as i64, d.axis_format.as_deref()),
        );
        tick += tick_step;
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
                    axis_y + AXIS_H,
                    day_w,
                    body_h,
                    "fill=\"#000\" fill-opacity=\"0.04\"",
                );
            }
        }
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
            svg.line(x, axis_y, x, HEADER_H + AXIS_H + body_h, &attrs);
            svg.text(
                x + 4.0,
                axis_y + 12.0,
                "fill=\"#d33\" font-size=\"11\" font-weight=\"bold\"",
                "today",
            );
        }
    }

    // Body
    let mut y = HEADER_H + AXIS_H;
    let mut flat_idx = 0;
    for section in &d.sections {
        if !section.name.is_empty() {
            svg.text(
                PAD,
                y + 16.0,
                &format!("fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
                &section.name,
            );
            y += SECTION_H;
        }
        for task in &section.tasks {
            let r = &resolved[flat_idx];
            // Task name in left column
            svg.text(
                PAD,
                y + 14.0,
                &format!("fill=\"{fg}\" font-size=\"12\""),
                &task.name,
            );
            let x = body_x + ((r.start_day - start_day) / total_days) * body_w;
            let (fill, stroke) = colors_for(task.status, task.crit);
            let sw = if task.crit { 2 } else { 1 };
            if let Some(click) = &task.click {
                open_click(&mut svg, click);
            }
            if task.vert {
                // Vertical marker line spanning the whole chart at the start
                // date; duration is ignored (the label sits beside the line).
                svg.line(
                    x,
                    axis_y,
                    x,
                    HEADER_H + AXIS_H + body_h,
                    &format!("stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-dasharray=\"2 2\""),
                );
                svg.text(
                    x + 4.0,
                    y + 14.0,
                    &format!("fill=\"{fg}\" font-size=\"11\""),
                    &task.name,
                );
            } else if task.milestone {
                // Diamond centered on the start date; duration is ignored.
                let cy = y + BAR_H / 2.0;
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
                    &format!("fill=\"{fg}\" font-size=\"11\""),
                    &task.name,
                );
            } else {
                // Bar
                let w = (r.duration / total_days) * body_w;
                svg.rect(
                    x,
                    y + 2.0,
                    w.max(2.0),
                    BAR_H - 4.0,
                    &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\" rx=\"3\""),
                );
            }
            if let Some(click) = &task.click {
                close_click(&mut svg, click);
            }
            y += BAR_H + ROW_GAP;
            flat_idx += 1;
        }
    }

    svg.finish()
}

struct Resolved {
    start_day: f64,
    duration: f64,
}

fn resolve_tasks(d: &GanttDiagram) -> Vec<Resolved> {
    // Flat order, accumulate `last_end` for `AfterPrevious`. Dates are exact
    // day counts from the Unix epoch (see `gantt_date`); `excludes` stretches
    // duration-based tasks over non-working days like upstream does.
    let df = d.date_format.as_deref();
    let excludes = Excludes::parse(&d.excludes, df, d.weekend.as_deref());
    let mut out: Vec<Resolved> = Vec::new();
    let mut id_to_start_end: HashMap<String, (f64, f64)> = HashMap::new();
    let mut last_end = 0.0_f64;

    for section in &d.sections {
        for task in &section.tasks {
            let start = match &task.start {
                TaskStart::Date(s) => parse_datetime(s, df).unwrap_or(last_end),
                // `after a b …` starts at the *latest* end of the named
                // predecessors; unknown ids are ignored, and if none resolve
                // it falls back to the previous task's end (like a single ref).
                TaskStart::AfterId(ids) => {
                    let latest = ids
                        .iter()
                        .filter_map(|id| id_to_start_end.get(id).map(|(_, e)| *e))
                        .fold(f64::NEG_INFINITY, f64::max);
                    if latest.is_finite() {
                        latest
                    } else {
                        last_end
                    }
                }
                TaskStart::AfterPrevious => last_end,
            };
            // `until <id>` ends where the named task starts; an end date ends
            // there directly. Both fall back to a nominal length when the
            // reference is a forward/unknown ref (matching `after`'s fallback).
            let dur = match &task.end {
                TaskEnd::Duration(days) => stretched_duration(start, *days, &excludes),
                TaskEnd::Date(s) => parse_datetime(s, df).map(|e| e - start).unwrap_or(1.0),
                TaskEnd::UntilId(id) => id_to_start_end
                    .get(id)
                    .map(|(s, _)| *s - start)
                    .unwrap_or(1.0),
            }
            .max(0.5);
            let end = start + dur;
            if let Some(id) = &task.id {
                id_to_start_end.insert(id.clone(), (start, end));
            }
            last_end = end;
            out.push(Resolved {
                start_day: start,
                duration: dur,
            });
        }
    }
    out
}

/// Length in calendar days for a `days`-working-day duration: when excludes is
/// active the whole-day part is stretched over non-working days, keeping any
/// sub-day (hours/minutes) remainder as-is.
fn stretched_duration(start: f64, days: f64, excludes: &Excludes) -> f64 {
    if !excludes.active() || days < 1.0 {
        return days;
    }
    let whole = days.floor() as i64;
    let end = excludes.stretched_end(start.round() as i64, whole);
    (end as f64 - start) + (days - whole as f64)
}

/// Days per tick for a `tickInterval` value like `1day`, `2week`, `1month`
/// (also the bare `1d`/`1w` units). Returns `None` for an unrecognized unit.
fn parse_tick_interval(s: &str) -> Option<f64> {
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
fn weekday_tick_offset(weekday_name: Option<&str>, start_day: f64) -> f64 {
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
fn css_style(css: &str) -> String {
    let joined = css
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    escape(&joined)
}

fn pick_tick_step(total_days: f64) -> f64 {
    if total_days <= 7.0 {
        1.0
    } else if total_days <= 30.0 {
        2.0
    } else if total_days <= 120.0 {
        7.0
    } else {
        30.0
    }
}

fn colors_for(status: TaskStatus, crit: bool) -> (&'static str, &'static str) {
    let (mut fill, mut stroke) = match status {
        TaskStatus::Normal => ("#A8C5E1", "#5470C6"),
        TaskStatus::Active => ("#FAC858", "#C99A3D"),
        TaskStatus::Done => ("#B8D8B8", "#73A573"),
    };
    if crit {
        // `crit` adds a red border; a crit-only task also takes the red fill,
        // while `done, crit` / `active, crit` keep their status fill.
        if status == TaskStatus::Normal {
            fill = "#F19E9E";
        }
        stroke = "#C0524F";
    }
    (fill, stroke)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> GanttDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Gantt(g) => g,
            _ => panic!("not gantt"),
        }
    }

    #[test]
    fn renders_basic() {
        let d = build(
            "gantt\ntitle My Plan\ndateFormat YYYY-MM-DD\nsection Phase 1\nDesign : a1, 2026-01-01, 5d\nReview : after a1, 2d\n",
        );
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">My Plan<"));
        assert!(svg.contains(">Phase 1<"));
        assert!(svg.contains(">Design<"));
        assert!(svg.contains(">Review<"));
    }

    #[test]
    fn task_after_id_starts_right_of_predecessor() {
        let d = build("gantt\nsection S\nA : a, 2026-01-01, 5d\nB : after a, 3d\n");
        let resolved = resolve_tasks(&d);
        assert!(resolved[1].start_day >= resolved[0].start_day + resolved[0].duration - 1e-6);
    }

    #[test]
    fn milestone_renders_diamond_not_bar() {
        let d = build("gantt\nsection S\nKickoff : milestone, 2026-01-01, 0d\n");
        let svg = render(&d, &Theme::default());
        // Diamond is drawn as a <path> with a Z-closed rhombus, no bar <rect>.
        assert!(svg.contains("<path"));
        assert!(svg.contains(">Kickoff<"));
    }

    #[test]
    fn end_date_sets_bar_length() {
        // Two days between 2014-01-06 and 2014-01-08.
        let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nT : a1, 2014-01-06, 2014-01-08\n");
        let resolved = resolve_tasks(&d);
        assert!((resolved[0].duration - 2.0).abs() < 1e-6);
    }

    #[test]
    fn until_ends_at_referenced_task_start() {
        // B starts four days before A and runs `until A`, so it ends where A
        // starts — a 4-day bar.
        let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nA : a, 2014-01-05, 5d\nB : b, 2014-01-01, until a\n");
        let resolved = resolve_tasks(&d);
        assert!((resolved[1].duration - 4.0).abs() < 1e-6);
        assert!(
            (resolved[1].start_day + resolved[1].duration - resolved[0].start_day).abs() < 1e-6
        );
    }

    #[test]
    fn crit_uses_red_palette() {
        let d = build("gantt\nsection S\nUrgent : crit, 2026-01-01, 1d\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("#F19E9E") || svg.contains("#C0524F"));
    }

    #[test]
    fn done_crit_keeps_done_fill_with_red_border() {
        let d = build("gantt\nsection S\nT : done, crit, 2026-01-01, 2d\n");
        let svg = render(&d, &Theme::default());
        // Done fill + crit border, not the crit red fill.
        assert!(svg.contains("#B8D8B8"));
        assert!(svg.contains("#C0524F"));
    }

    #[test]
    fn excludes_weekends_shade_and_stretch() {
        // 2026-01-01 is a Thursday; a 5-working-day task crosses the weekend,
        // so with `excludes weekends` the bar spans 7 calendar days.
        let d = build(
            "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection S\nT : 2026-01-01, 5d\n",
        );
        let resolved = resolve_tasks(&d);
        assert!((resolved[0].duration - 7.0).abs() < 1e-6);
        let svg = render(&d, &Theme::default());
        // Weekend shading band present.
        assert!(svg.contains("fill-opacity=\"0.04\""));
    }

    #[test]
    fn tick_interval_units() {
        assert_eq!(parse_tick_interval("1day"), Some(1.0));
        assert_eq!(parse_tick_interval("2week"), Some(14.0));
        assert_eq!(parse_tick_interval("1month"), Some(30.0));
        assert_eq!(parse_tick_interval("1w"), Some(7.0));
        assert_eq!(parse_tick_interval("1year"), None);
    }

    #[test]
    fn tick_interval_overrides_auto_step() {
        // A 28-day span auto-picks a 2-day step (14 labels); `tickInterval
        // 1week` forces a 7-day step (fewer labels).
        let span = "gantt\ndateFormat YYYY-MM-DD\nsection S\nT : 2026-01-01, 28d\n";
        let auto = render(&build(span), &Theme::default());
        let weekly = render(
            &build(&span.replace("section S", "tickInterval 1week\nsection S")),
            &Theme::default(),
        );
        let count = |s: &str| s.matches("font-size=\"11\"").count();
        assert!(count(&weekly) < count(&auto));
    }

    #[test]
    fn weekday_offset_is_days_to_next_named_weekday() {
        let thu = crate::svg::gantt_date::days_from_civil(2026, 1, 1) as f64;
        assert_eq!(weekday_tick_offset(Some("monday"), thu), 4.0);
        assert_eq!(weekday_tick_offset(Some("thursday"), thu), 0.0);
        assert_eq!(weekday_tick_offset(None, thu), 0.0);
    }

    #[test]
    fn weekend_friday_shades_friday_not_sunday() {
        // A task spanning the first week of 2026 with `weekend friday`: Friday
        // 2026-01-02 becomes a shaded non-working day.
        let d = build(
            "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nweekend friday\nsection S\nT : 2026-01-01, 10d\n",
        );
        let ex = Excludes::parse(&d.excludes, d.date_format.as_deref(), d.weekend.as_deref());
        use crate::svg::gantt_date::days_from_civil;
        assert!(ex.is_excluded(days_from_civil(2026, 1, 2))); // Friday
        assert!(!ex.is_excluded(days_from_civil(2026, 1, 4))); // Sunday now works
    }

    #[test]
    fn vert_task_draws_marker_line_not_bar() {
        let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nBase : 2026-01-01, 10d\nFreeze : vert, v1, 2026-01-05, 0d\n");
        let svg = render(&d, &Theme::default());
        // The vertical marker is a dashed <line>; the label is drawn.
        assert!(svg.contains("stroke-dasharray=\"2 2\""));
        assert!(svg.contains(">Freeze<"));
    }

    #[test]
    fn click_wraps_task_bar_in_anchor() {
        let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nA : a, 2026-01-01, 5d\nclick a href \"https://example.com\"\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("<a href=\"https://example.com\""));
    }

    #[test]
    fn today_marker_off_draws_no_marker() {
        // A chart around the present day with `todayMarker off` shows no marker.
        let base = today_days();
        use crate::svg::gantt_date::civil_from_days;
        let (y, m, day) = civil_from_days(base - 1);
        let src = format!(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection S\nT : {y:04}-{m:02}-{day:02}, 5d\n"
        );
        let svg = render(&build(&src), &Theme::default());
        assert!(!svg.contains(">today<"));
    }

    #[test]
    fn today_marker_style_applied_when_in_range() {
        let base = today_days();
        use crate::svg::gantt_date::civil_from_days;
        let (y, m, day) = civil_from_days(base - 1);
        let src = format!(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker stroke:cyan,stroke-width:5px\nsection S\nT : {y:04}-{m:02}-{day:02}, 5d\n"
        );
        let svg = render(&build(&src), &Theme::default());
        assert!(svg.contains(">today<"));
        assert!(svg.contains("stroke:cyan; stroke-width:5px"));
    }

    #[test]
    fn after_multiple_ids_uses_latest_end() {
        // C follows the later of A (ends day 5) and B (ends day 10).
        let d = build(
            "gantt\ndateFormat YYYY-MM-DD\nsection S\nA : a, 2026-01-01, 5d\nB : b, 2026-01-01, 10d\nC : after a b, 2d\n",
        );
        let resolved = resolve_tasks(&d);
        let b_end = resolved[1].start_day + resolved[1].duration;
        assert!((resolved[2].start_day - b_end).abs() < 1e-6);
    }

    #[test]
    fn axis_format_controls_tick_labels() {
        let d = build(
            "gantt\ndateFormat YYYY-MM-DD\naxisFormat %m/%d\nsection S\nT : 2026-01-01, 3d\n",
        );
        let svg = render(&d, &Theme::default());
        // A `%m/%d` axis label like `01/01`, and no ISO `2026-01-01` tick.
        assert!(svg.contains(">01/01<"));
        assert!(!svg.contains(">2026-01-01<"));
    }
}
