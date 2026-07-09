//! Task scheduling and per-status colors for the gantt renderer.
//!
//! Resolves each task's absolute start (in "days from epoch") and duration,
//! computes the overall chart span, and maps `(status, crit)` to bar colors.

use std::collections::HashMap;

use crate::parse::{GanttDiagram, TaskEnd, TaskStart, TaskStatus};

use crate::svg::gantt_date::{parse_datetime, Excludes};

pub(super) struct Resolved {
    pub(super) start_day: f64,
    pub(super) duration: f64,
}

pub(super) fn resolve_tasks(d: &GanttDiagram) -> Vec<Resolved> {
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
            // Guard only against a negative length here; the visible-bar floor
            // is applied at render time so sub-day charts keep true durations.
            .max(0.0);
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

/// Project time span as `(start_day, total_days, sub_day)`. The raw (unclamped)
/// span decides `sub_day`: a chart shorter than a day drops the half-day bar
/// floor and lets the axis span the true sub-day range instead of stretching to
/// one full day.
pub(super) fn chart_span(resolved: &[Resolved]) -> (f64, f64, bool) {
    let project_start = resolved
        .iter()
        .map(|t| t.start_day)
        .fold(f64::INFINITY, f64::min);
    let raw_end = resolved
        .iter()
        .map(|t| t.start_day + t.duration)
        .fold(f64::NEG_INFINITY, f64::max);
    let raw_span = if project_start.is_finite() && raw_end.is_finite() {
        raw_end - project_start
    } else {
        0.0
    };
    let sub_day = raw_span > 0.0 && raw_span < 1.0;
    let min_bar_dur = if sub_day { 0.0 } else { 0.5 };
    let project_end = resolved
        .iter()
        .map(|t| t.start_day + t.duration.max(min_bar_dur))
        .fold(f64::NEG_INFINITY, f64::max);
    if project_start.is_finite() && project_end.is_finite() {
        let floor = if sub_day { raw_span } else { 1.0 };
        (
            project_start,
            (project_end - project_start).max(floor),
            sub_day,
        )
    } else {
        (0.0, 1.0, sub_day)
    }
}

/// Task bar fill/border, matching upstream's default gantt theme
/// (`taskBkgColor`/`taskBorderColor` families): purple normal, pale
/// lavender-blue active, light-grey done, solid-red crit.
pub(super) fn colors_for(status: TaskStatus, crit: bool) -> (&'static str, &'static str) {
    let (mut fill, mut stroke) = match status {
        TaskStatus::Normal => ("#8a90dd", "#534fbc"),
        TaskStatus::Active => ("#bfc7ff", "#534fbc"),
        TaskStatus::Done => ("#d3d3d3", "#808080"),
    };
    if crit {
        // `crit` adds the light-red border; a crit-only task also takes the
        // solid-red fill, while `done, crit` / `active, crit` keep their status
        // fill (upstream `activeCrit`/`doneCrit`).
        if status == TaskStatus::Normal {
            fill = "red";
        }
        stroke = "#ff8888";
    }
    (fill, stroke)
}

/// Ink for a label drawn *inside* a bar. Upstream's default `.taskText` is white
/// (used by normal and crit-only bars over their dark/red fills), while `active`
/// and `done` bars override to the dark `taskTextDarkColor`. Labels drawn to the
/// right of a bar always use the dark outside color (the caller passes `fg`).
pub(super) fn inside_text_ink(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Normal => "#fff",
        TaskStatus::Active | TaskStatus::Done => "#333",
    }
}
