//! Gantt chart parser (v0.1 subset).
//!
//! Supports:
//!   * Header: `gantt`.
//!   * `title <text>`, `dateFormat <fmt>`, `axisFormat <fmt>`.
//!   * `section <name>` blocks.
//!   * Tasks: `<name> : [tags,] [id,] <start>, <end>` — the end may be a
//!     duration (`Nd`/`Nw`/`Nh`/`Nm`), an end date, or `until <taskId>`.
//!     A single time token (`<name> : 24d` / `<name> : until id`) is a
//!     duration/until with an implicit start at the previous task's end.
//!     Tags ⊆ {active, done, crit, milestone} (any combination), start is a
//!     date or `after <id>`. `milestone` renders a diamond at the start date
//!     (the end is ignored).

use super::ast::{GanttDiagram, GanttSection, GanttTask, TaskEnd, TaskStart, TaskStatus};
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
        if line.starts_with("includes ")
            || line.starts_with("weekday ")
            || line.starts_with("tickInterval ")
            || line == "inclusiveEndDates"
            || line == "topAxis"
        {
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
    if parts.is_empty() {
        return Err(ParseError::Syntax {
            message: "task needs at least an end (<duration>/<end date>/until <id>)".into(),
            line: line_no,
        });
    }

    let mut status = TaskStatus::Normal;
    let mut milestone = false;
    let mut id: Option<String> = None;
    let mut consumed = 0;

    // Leading tags (optional, any combination): `active`/`done`/`crit` set the
    // status, `milestone` is orthogonal. Upstream allows e.g. `crit, milestone`.
    while consumed < parts.len() {
        match parts[consumed] {
            "milestone" => milestone = true,
            "active" => status = TaskStatus::Active,
            "done" => status = TaskStatus::Done,
            "crit" => status = TaskStatus::Crit,
            _ => break,
        }
        consumed += 1;
    }

    // The time spec is the trailing 1-2 tokens; anything before it is the id.
    // A leading id is present only when it isn't itself a time token and at
    // least one time token follows it.
    if parts.len() - consumed >= 2 {
        let candidate = parts[consumed];
        if !looks_like_start(candidate) && !looks_like_end(candidate) {
            id = Some(candidate.to_string());
            consumed += 1;
        }
    }

    if id.is_none() {
        *auto_id_counter += 1;
        id = Some(format!("task{auto_id_counter}"));
    }

    let spec = &parts[consumed..];
    let (start, end) = match spec {
        // `<start>, <end>` — start is a date or `after <id>`, end is a
        // duration, an end date, or `until <id>`.
        [start_raw, end_raw, ..] => {
            let start = parse_start(start_raw, last_task_id);
            let end = parse_end(end_raw, line_no)?;
            (start, end)
        }
        // Single time token — duration/until with an implicit start at the
        // previous task's end.
        [only] => (TaskStart::AfterPrevious, parse_end(only, line_no)?),
        [] => {
            return Err(ParseError::Syntax {
                message: "missing task end (<duration>/<end date>/until <id>)".into(),
                line: line_no,
            });
        }
    };

    Ok(GanttTask {
        name: name.to_string(),
        id,
        start,
        end,
        status,
        milestone,
    })
}

fn parse_start(start_raw: &str, last_task_id: Option<&str>) -> TaskStart {
    if let Some(after) = start_raw.strip_prefix("after ") {
        TaskStart::AfterId(after.trim().to_string())
    } else if start_raw.is_empty() && last_task_id.is_some() {
        TaskStart::AfterPrevious
    } else {
        TaskStart::Date(start_raw.to_string())
    }
}

fn parse_end(s: &str, line_no: usize) -> Result<TaskEnd, ParseError> {
    if let Some(dur) = parse_duration(s) {
        Ok(TaskEnd::Duration(dur))
    } else if let Some(id) = s.strip_prefix("until ") {
        Ok(TaskEnd::UntilId(id.trim().to_string()))
    } else if looks_like_date(s) {
        Ok(TaskEnd::Date(s.to_string()))
    } else {
        Err(ParseError::Syntax {
            message: format!("cannot parse task end: '{s}'"),
            line: line_no,
        })
    }
}

fn looks_like_date(s: &str) -> bool {
    s.chars().filter(|c| *c == '-').count() >= 2
}

fn looks_like_start(s: &str) -> bool {
    s.starts_with("after ") || looks_like_date(s)
}

/// A token that ends a task: a duration, `until <id>`, or an end date.
fn looks_like_end(s: &str) -> bool {
    parse_duration(s).is_some() || s.starts_with("until ") || looks_like_date(s)
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
        assert_eq!(design.end, TaskEnd::Duration(5.0));
        let review = &d.sections[0].tasks[1];
        match &review.start {
            TaskStart::AfterId(s) => assert_eq!(s, "a1"),
            _ => panic!("expected after id"),
        }
        let build = &d.sections[1].tasks[0];
        assert_eq!(build.status, TaskStatus::Crit);
        assert_eq!(build.end, TaskEnd::Duration(7.0));
    }

    #[test]
    fn end_date_form() {
        // `<start>, <end date>` instead of a duration.
        let d = parse("gantt\nsection S\nTask : a1, 2014-01-06, 2014-01-08\n").unwrap();
        let t = &d.sections[0].tasks[0];
        assert_eq!(t.id.as_deref(), Some("a1"));
        assert_eq!(t.start, TaskStart::Date("2014-01-06".into()));
        assert_eq!(t.end, TaskEnd::Date("2014-01-08".into()));
    }

    #[test]
    fn duration_only_implicit_start() {
        // A single time token starts at the previous task's end.
        let d = parse("gantt\nsection S\nFirst : 2014-01-01, 5d\nanother task : 24d\n").unwrap();
        let t = &d.sections[0].tasks[1];
        assert_eq!(t.start, TaskStart::AfterPrevious);
        assert_eq!(t.end, TaskEnd::Duration(24.0));
    }

    #[test]
    fn until_end_marker() {
        let d = parse("gantt\nsection S\nA : 2014-01-01, 5d\nB : after A, until A\n").unwrap();
        let b = &d.sections[0].tasks[1];
        assert_eq!(b.end, TaskEnd::UntilId("A".into()));
        // Single-token `until` form.
        let d = parse("gantt\nsection S\nA : 2014-01-01, 5d\nB : until A\n").unwrap();
        let b = &d.sections[0].tasks[1];
        assert_eq!(b.start, TaskStart::AfterPrevious);
        assert_eq!(b.end, TaskEnd::UntilId("A".into()));
    }

    #[test]
    fn config_keywords_do_not_error() {
        let d = parse(
            "gantt\ndateFormat YYYY-MM-DD\ntickInterval 1week\ninclusiveEndDates\ntopAxis\nsection S\nA : 2014-01-01, 5d\n",
        )
        .unwrap();
        assert_eq!(d.sections[0].tasks.len(), 1);
    }

    #[test]
    fn milestone_tag_parsed_and_combinable() {
        let s = "gantt\nsection S\n\
                 M1 : milestone, 2026-01-06, 0d\n\
                 M2 : crit, milestone, m2, 2026-01-08, 0d\n";
        let d = parse(s).unwrap();
        let m1 = &d.sections[0].tasks[0];
        assert!(m1.milestone);
        assert_eq!(m1.status, TaskStatus::Normal);
        let m2 = &d.sections[0].tasks[1];
        assert!(m2.milestone);
        assert_eq!(m2.status, TaskStatus::Crit);
        assert_eq!(m2.id.as_deref(), Some("m2"));
    }

    #[test]
    fn auto_id_when_omitted() {
        let s = "gantt\nsection S\nA : 2026-01-01, 3d\nB : 2026-01-05, 2d\n";
        let d = parse(s).unwrap();
        let ids: Vec<_> = d.sections[0].tasks.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids, vec![Some("task1".into()), Some("task2".into())]);
    }
}
