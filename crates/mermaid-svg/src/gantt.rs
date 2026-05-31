//! Gantt chart renderer.
//!
//! Computes per-task absolute start (in "days from project start") by resolving
//! `after <id>` and `AfterPrevious` references, then lays out one bar per task
//! in vertical sequence with a time axis at the top.

use std::collections::HashMap;

use mermaid_parse::{GanttDiagram, TaskStart, TaskStatus};

use crate::svg::SvgBuilder;
use crate::theme::FG;

const LABEL_COL_W: f64 = 200.0;
const BAR_H: f64 = 20.0;
const ROW_GAP: f64 = 12.0;
const HEADER_H: f64 = 56.0;
const AXIS_H: f64 = 26.0;
const SECTION_H: f64 = 24.0;
const PAD: f64 = 16.0;
const TIME_COL_MIN_W: f64 = 480.0;

pub(crate) fn render(d: &GanttDiagram) -> String {
    // Step 1 — resolve absolute start positions in "days from epoch".
    let resolved = resolve_tasks(d);
    let project_start = resolved.iter().map(|t| t.start_day).fold(f64::INFINITY, f64::min);
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

    let mut svg = SvgBuilder::new(width, height);

    // Title
    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 20.0,
            &format!("text-anchor=\"middle\" fill=\"{FG}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let body_x = PAD + LABEL_COL_W;
    let body_w = time_col_w;
    let axis_y = HEADER_H;

    // Axis line + day ticks
    svg.line(body_x, axis_y + AXIS_H - 1.0, body_x + body_w, axis_y + AXIS_H - 1.0, "stroke=\"#999\" stroke-width=\"1\"");
    let tick_step = pick_tick_step(total_days);
    let mut tick = 0.0;
    while tick <= total_days + 1e-6 {
        let x = body_x + (tick / total_days) * body_w;
        svg.line(x, axis_y, x, axis_y + AXIS_H, "stroke=\"#bbb\" stroke-width=\"1\"");
        svg.text(
            x + 2.0,
            axis_y + 14.0,
            &format!("fill=\"{FG}\" font-size=\"11\""),
            &format_day(tick, start_day, d.date_format.as_deref()),
        );
        tick += tick_step;
    }

    // Body
    let mut y = HEADER_H + AXIS_H;
    let mut flat_idx = 0;
    for section in &d.sections {
        if !section.name.is_empty() {
            svg.text(
                PAD,
                y + 16.0,
                &format!("fill=\"{FG}\" font-size=\"13\" font-weight=\"bold\""),
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
                &format!("fill=\"{FG}\" font-size=\"12\""),
                &task.name,
            );
            // Bar
            let x = body_x + ((r.start_day - start_day) / total_days) * body_w;
            let w = (r.duration / total_days) * body_w;
            let (fill, stroke) = colors_for(task.status);
            svg.rect(
                x,
                y + 2.0,
                w.max(2.0),
                BAR_H - 4.0,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1\" rx=\"3\""),
            );
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
    // Flat order, accumulate a cursor for `AfterPrevious`.
    let mut out: Vec<Resolved> = Vec::new();
    let mut id_to_start_end: HashMap<String, (f64, f64)> = HashMap::new();
    let mut cursor = 0.0_f64;
    let mut last_end = 0.0_f64;

    // First pass: resolve sequentially, treating dates as opaque keys.
    // Real Mermaid parses YYYY-MM-DD dates; we use ordinal days from the
    // earliest known date.
    let mut date_keys: Vec<String> = Vec::new();
    for section in &d.sections {
        for task in &section.tasks {
            if let TaskStart::Date(s) = &task.start {
                if !date_keys.contains(s) {
                    date_keys.push(s.clone());
                }
            }
        }
    }
    let date_index: HashMap<String, f64> = date_keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i as f64 * 0.0))
        .collect();
    let _ = date_index;

    // Convert known YYYY-MM-DD via simple parsing; non-parseable dates become
    // 0 (so the chart still renders sensibly).
    fn ymd_to_day(s: &str) -> Option<f64> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let y: i32 = parts[0].parse().ok()?;
        let m: i32 = parts[1].parse().ok()?;
        let dd: i32 = parts[2].parse().ok()?;
        // Days from year 0 — only stability matters, not absolute correctness.
        Some((y as f64) * 365.25 + month_offset(m) + (dd as f64))
    }
    fn month_offset(m: i32) -> f64 {
        // Cumulative days at start of month (non-leap).
        const TBL: [f64; 12] = [0., 31., 59., 90., 120., 151., 181., 212., 243., 273., 304., 334.];
        TBL[(m.clamp(1, 12) - 1) as usize]
    }

    for section in &d.sections {
        for task in &section.tasks {
            let start = match &task.start {
                TaskStart::Date(s) => ymd_to_day(s).unwrap_or(cursor),
                TaskStart::AfterId(id) => id_to_start_end
                    .get(id)
                    .map(|(_, e)| *e)
                    .unwrap_or(last_end),
                TaskStart::AfterPrevious => last_end,
            };
            let dur = task.duration_days.max(0.5);
            let end = start + dur;
            if let Some(id) = &task.id {
                id_to_start_end.insert(id.clone(), (start, end));
            }
            cursor = end;
            last_end = end;
            out.push(Resolved {
                start_day: start,
                duration: dur,
            });
        }
    }
    out
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

fn format_day(day_offset: f64, start_day: f64, date_format: Option<&str>) -> String {
    // If start_day looks like a real ordinal (positive and large), convert back.
    if start_day > 365.0 {
        let abs = start_day + day_offset;
        // Reverse the (approximate) ymd encoding so we print something readable.
        // This is intentionally rough; switching to chrono would be cleaner.
        let y = (abs / 365.25).floor() as i32;
        let leftover = abs - (y as f64) * 365.25;
        let mut m = 1;
        let mut cum = 0.0_f64;
        const TBL: [f64; 12] = [31., 28., 31., 30., 31., 30., 31., 31., 30., 31., 30., 31.];
        while m < 12 && cum + TBL[(m - 1) as usize] <= leftover {
            cum += TBL[(m - 1) as usize];
            m += 1;
        }
        let dd = (leftover - cum).max(0.0).round() as i32 + 1;
        let _ = date_format;
        return format!("{y:04}-{m:02}-{dd:02}");
    }
    format!("d{}", day_offset as i32)
}

fn colors_for(status: TaskStatus) -> (&'static str, &'static str) {
    match status {
        TaskStatus::Normal => ("#A8C5E1", "#5470C6"),
        TaskStatus::Active => ("#FAC858", "#C99A3D"),
        TaskStatus::Done => ("#B8D8B8", "#73A573"),
        TaskStatus::Crit => ("#F19E9E", "#C0524F"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_parse::parse;

    fn build(s: &str) -> GanttDiagram {
        match parse(s).unwrap() {
            mermaid_parse::Diagram::Gantt(g) => g,
            _ => panic!("not gantt"),
        }
    }

    #[test]
    fn renders_basic() {
        let d = build(
            "gantt\ntitle My Plan\ndateFormat YYYY-MM-DD\nsection Phase 1\nDesign : a1, 2026-01-01, 5d\nReview : after a1, 2d\n",
        );
        let svg = render(&d);
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
    fn crit_uses_red_palette() {
        let d = build("gantt\nsection S\nUrgent : crit, 2026-01-01, 1d\n");
        let svg = render(&d);
        assert!(svg.contains("#F19E9E") || svg.contains("#C0524F"));
    }
}
