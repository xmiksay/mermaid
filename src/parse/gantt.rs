//! Gantt chart parser (v0.1 subset).
//!
//! Supports:
//!   * Header: `gantt`.
//!   * `title <text>`, `dateFormat <fmt>`, `axisFormat <fmt>`.
//!   * `section <name>` blocks.
//!   * Tasks: `<name> : [status,] [id,] <start>, <duration>`
//!     where status ∈ {active, done, crit}, start is a date or `after <id>`,
//!     and duration is `Nd` / `Nw` / `Nh` / a bare date.

use super::ast::{GanttDiagram, GanttSection, GanttTask, TaskStart, TaskStatus};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<GanttDiagram, ParseError> {
    let mut diag = GanttDiagram::default();
    let mut header_seen = false;
    let mut last_task_id: Option<String> = None;
    let mut auto_id_counter = 0usize;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "gantt" {
                return Err(ParseError::Syntax {
                    message: "expected 'gantt' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title ") {
            diag.title = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("dateFormat ") {
            diag.date_format = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("axisFormat ") {
            diag.axis_format = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("excludes ") {
            for tok in rest.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                diag.excludes.push(tok.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("todayMarker ") {
            diag.today_marker = Some(rest.trim().to_string());
            continue;
        }
        // `today YYYY-MM-DD` shorthand used by some examples.
        if let Some(rest) = line.strip_prefix("today ") {
            diag.today_marker = Some(rest.trim().to_string());
            continue;
        }
        if line.starts_with("includes ") || line.starts_with("weekday ") {
            // Accepted but currently informational only.
            continue;
        }
        if let Some(rest) = line.strip_prefix("section ") {
            diag.sections.push(GanttSection {
                name: rest.trim().to_string(),
                tasks: Vec::new(),
            });
            continue;
        }

        // Task line: `<name> : <fields>`
        let (name, fields) = match line.split_once(':') {
            Some(t) => t,
            None => {
                return Err(ParseError::Syntax {
                    message: format!("unrecognized gantt line: '{line}'"),
                    line: line_no,
                });
            }
        };

        // Ensure a section exists.
        if diag.sections.is_empty() {
            diag.sections.push(GanttSection {
                name: String::new(),
                tasks: Vec::new(),
            });
        }
        let section = diag.sections.last_mut().unwrap();
        let task = parse_task(
            name.trim(),
            fields.trim(),
            &mut auto_id_counter,
            last_task_id.as_deref(),
            line_no,
        )?;
        last_task_id = task.id.clone();
        section.tasks.push(task);
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

fn parse_task(
    name: &str,
    fields: &str,
    auto_id_counter: &mut usize,
    last_task_id: Option<&str>,
    line_no: usize,
) -> Result<GanttTask, ParseError> {
    let parts: Vec<&str> = fields.split(',').map(str::trim).collect();
    if parts.len() < 2 {
        return Err(ParseError::Syntax {
            message: "task needs at least <start>, <duration>".into(),
            line: line_no,
        });
    }

    let mut status = TaskStatus::Normal;
    let mut id: Option<String> = None;
    let mut consumed = 0;

    // Status (optional, must be the first token if present).
    if let Some(s) = parse_status(parts[0]) {
        status = s;
        consumed += 1;
    }
    // ID (optional). Heuristic: if the next token isn't a date/after/duration, treat as id.
    if consumed < parts.len() - 2 {
        let candidate = parts[consumed];
        if !looks_like_start(candidate) && !looks_like_duration(candidate) {
            id = Some(candidate.to_string());
            consumed += 1;
        }
    }

    if id.is_none() {
        *auto_id_counter += 1;
        id = Some(format!("task{auto_id_counter}"));
    }

    if consumed + 2 > parts.len() {
        return Err(ParseError::Syntax {
            message: "missing start or duration".into(),
            line: line_no,
        });
    }

    let start_raw = parts[consumed];
    let dur_raw = parts[consumed + 1];

    let start = if let Some(after) = start_raw.strip_prefix("after ") {
        TaskStart::AfterId(after.trim().to_string())
    } else if start_raw.is_empty() && last_task_id.is_some() {
        TaskStart::AfterPrevious
    } else {
        TaskStart::Date(start_raw.to_string())
    };

    let duration_days = parse_duration(dur_raw).ok_or_else(|| ParseError::Syntax {
        message: format!("cannot parse duration: '{dur_raw}'"),
        line: line_no,
    })?;

    Ok(GanttTask {
        name: name.to_string(),
        id,
        start,
        duration_days,
        status,
    })
}

fn parse_status(s: &str) -> Option<TaskStatus> {
    match s {
        "active" => Some(TaskStatus::Active),
        "done" => Some(TaskStatus::Done),
        "crit" => Some(TaskStatus::Crit),
        _ => None,
    }
}

fn looks_like_start(s: &str) -> bool {
    s.starts_with("after ") || s.chars().filter(|c| *c == '-').count() >= 2
}

fn looks_like_duration(s: &str) -> bool {
    parse_duration(s).is_some()
}

fn parse_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    let (num_part, unit) = if let Some(rest) = s.strip_suffix('d') {
        (rest, 1.0)
    } else if let Some(rest) = s.strip_suffix('w') {
        (rest, 7.0)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest, 1.0 / 24.0)
    } else if let Some(rest) = s.strip_suffix('m') {
        (rest, 1.0 / 1440.0)
    } else {
        return None;
    };
    num_part.parse::<f64>().ok().map(|n| n * unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_example() {
        let s = "gantt\n\
                 title My Project\n\
                 dateFormat YYYY-MM-DD\n\
                 section Planning\n\
                 Design : a1, 2026-01-01, 5d\n\
                 Review : after a1, 2d\n\
                 section Execution\n\
                 Build : crit, b1, 2026-01-08, 1w\n";
        let d = parse(s).unwrap();
        assert_eq!(d.title.as_deref(), Some("My Project"));
        assert_eq!(d.date_format.as_deref(), Some("YYYY-MM-DD"));
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[0].tasks.len(), 2);
        let design = &d.sections[0].tasks[0];
        assert_eq!(design.id.as_deref(), Some("a1"));
        assert_eq!(design.duration_days, 5.0);
        let review = &d.sections[0].tasks[1];
        match &review.start {
            TaskStart::AfterId(s) => assert_eq!(s, "a1"),
            _ => panic!("expected after id"),
        }
        let build = &d.sections[1].tasks[0];
        assert_eq!(build.status, TaskStatus::Crit);
        assert_eq!(build.duration_days, 7.0);
    }

    #[test]
    fn auto_id_when_omitted() {
        let s = "gantt\nsection S\nA : 2026-01-01, 3d\nB : 2026-01-05, 2d\n";
        let d = parse(s).unwrap();
        let ids: Vec<_> = d.sections[0].tasks.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids, vec![Some("task1".into()), Some("task2".into())]);
    }
}
