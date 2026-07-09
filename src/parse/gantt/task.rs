//! Task-line parsing: leading tags, optional id, and the start/end time spec.

use crate::parse::ast::{GanttTask, TaskEnd, TaskStart, TaskStatus};
use crate::parse::ParseError;

use super::tokens::{looks_like_date, looks_like_end, looks_like_start, parse_duration};

pub(super) fn parse_task(
    name: &str,
    fields: &str,
    auto_id_counter: &mut usize,
    last_task_id: Option<&str>,
    line_no: usize,
) -> Result<GanttTask, ParseError> {
    let parts: Vec<&str> = fields.split(',').map(str::trim).collect();
    if parts.is_empty() {
        return Err(ParseError::malformed(
            line_no,
            "task needs at least an end (<duration>/<end date>/until <id>)",
        ));
    }

    let mut status = TaskStatus::Normal;
    let mut crit = false;
    let mut milestone = false;
    let mut vert = false;
    let mut id: Option<String> = None;
    let mut consumed = 0;

    // Leading tags (optional, any combination): `active`/`done` set the status;
    // `crit`, `milestone` and `vert` are orthogonal flags. Upstream combines
    // them, e.g. `done, crit` keeps the done fill with a crit border rather than
    // letting the last tag win.
    while consumed < parts.len() {
        match parts[consumed] {
            "milestone" => milestone = true,
            "vert" => vert = true,
            "active" => status = TaskStatus::Active,
            "done" => status = TaskStatus::Done,
            "crit" => crit = true,
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
            return Err(ParseError::malformed(
                line_no,
                "missing task end (<duration>/<end date>/until <id>)",
            ));
        }
    };

    Ok(GanttTask {
        name: name.to_string(),
        id,
        start,
        end,
        status,
        crit,
        milestone,
        vert,
        click: None,
    })
}

fn parse_start(start_raw: &str, last_task_id: Option<&str>) -> TaskStart {
    if let Some(after) = start_raw.strip_prefix("after ") {
        // `after a b c` — space-separated list of predecessor ids.
        let ids: Vec<String> = after.split_whitespace().map(str::to_string).collect();
        TaskStart::AfterId(ids)
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
        Err(ParseError::number(
            line_no,
            format!("cannot parse task end: '{s}'"),
        ))
    }
}
