//! Gantt chart parser (v0.1 subset).
//!
//! Supports:
//!   * Header: `gantt`.
//!   * `title <text>`, `dateFormat <fmt>`, `axisFormat <fmt>`.
//!   * `excludes <days>`, `weekend friday|saturday` (redefines the weekend
//!     pair), `tickInterval Nday|Nweek|Nmonth`, `weekday <day>`,
//!     `displayMode[:] compact` (stored; layout is a follow-up).
//!   * `section <name>` blocks.
//!   * Tasks: `<name> : [tags,] [id,] <start>, <end>` — the end may be a
//!     duration (units `ms`/`s`/`m`/`h`/`d`/`w`/`M`/`y`, decimals allowed), an
//!     end date, or `until <taskId>`. A single time token (`<name> : 24d` /
//!     `<name> : until id`) is a duration/until with an implicit start at the
//!     previous task's end. Tags ⊆ {active, done, crit, milestone, vert} (any
//!     combination), start is a date or `after <id> [<id> …]`. `milestone`
//!     renders a diamond at the start date; `vert` a vertical marker line (both
//!     ignore the end).
//!   * `click <taskId> href "url"` / `click <taskId> call fn()` — binds an
//!     interaction to a task (shared with the flowchart `click` parser).

use std::collections::HashMap;

use super::ast::{ClickAction, GanttDiagram, GanttSection};
use super::flowchart::click::parse_click;
use super::{strip_comment, ParseError};

use task::parse_task;
use tokens::strip_kw;

mod task;
mod tokens;

#[cfg(test)]
mod tests;

pub(crate) fn parse(input: &str) -> Result<GanttDiagram, ParseError> {
    let mut diag = GanttDiagram::default();
    let mut header_seen = false;
    let mut last_task_id: Option<String> = None;
    let mut auto_id_counter = 0usize;
    let mut clicks: HashMap<String, ClickAction> = HashMap::new();

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "gantt" {
                return Err(ParseError::header(line_no, "expected 'gantt' header"));
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
            // The value is a CSS style string (or `off` to hide it), not a
            // date — the marker is always positioned at the *current* date.
            diag.today_marker = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("click ") {
            let (id, action) = parse_click(rest).ok_or_else(|| {
                ParseError::malformed(line_no, "malformed gantt 'click' statement")
            })?;
            clicks.insert(id, action);
            continue;
        }
        if let Some(rest) = strip_kw(line, "weekend") {
            diag.weekend = Some(rest.to_string());
            continue;
        }
        if let Some(rest) = strip_kw(line, "weekday") {
            diag.weekday = Some(rest.to_string());
            continue;
        }
        if let Some(rest) = strip_kw(line, "tickInterval") {
            diag.tick_interval = Some(rest.to_string());
            continue;
        }
        if let Some(rest) = strip_kw(line, "displayMode") {
            diag.display_mode = Some(rest.to_string());
            continue;
        }
        if line == "topAxis" {
            diag.top_axis = true;
            continue;
        }
        if line.starts_with("includes ") || line == "inclusiveEndDates" {
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
                return Err(ParseError::unknown(
                    line_no,
                    format!("unrecognized gantt line: '{line}'"),
                ));
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

    // Bind collected `click` directives onto their tasks by id (a directive may
    // appear before or after the task it targets).
    if !clicks.is_empty() {
        for section in &mut diag.sections {
            for task in &mut section.tasks {
                if let Some(id) = &task.id {
                    if let Some(action) = clicks.remove(id) {
                        task.click = Some(action);
                    }
                }
            }
        }
    }

    Ok(diag)
}
