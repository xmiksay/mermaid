//! Kanban parser. Indentation-based:
//!
//! ```text
//! kanban
//!   Todo
//!     [Task 1]@{ assigned: 'Alice', priority: 'High' }
//!     Task 2
//!   Doing
//!     [Task 3]
//!   Done
//!     [Task 4]
//! ```

use super::ast::{KanbanColumn, KanbanDiagram, KanbanTask};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<KanbanDiagram, ParseError> {
    let mut d = KanbanDiagram::default();
    let mut header_seen = false;
    let mut current_col: Option<KanbanColumn> = None;
    let mut col_indent: Option<usize> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let content = strip_comment(raw);
        if content.trim().is_empty() {
            continue;
        }
        if !header_seen {
            if content.trim() != "kanban" {
                return Err(ParseError::Syntax {
                    message: "expected 'kanban' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        let indent = content
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let body = content.trim();
        if col_indent.is_none() {
            col_indent = Some(indent);
        }

        if indent == col_indent.unwrap() {
            if let Some(c) = current_col.take() {
                d.columns.push(c);
            }
            current_col = Some(KanbanColumn {
                id: body.to_string(),
                label: body.to_string(),
                tasks: Vec::new(),
            });
        } else {
            let task = parse_task(body);
            if let Some(c) = current_col.as_mut() {
                c.tasks.push(task);
            } else {
                let mut col = KanbanColumn {
                    id: "Default".into(),
                    label: "Default".into(),
                    tasks: Vec::new(),
                };
                col.tasks.push(task);
                current_col = Some(col);
            }
        }
    }

    if let Some(c) = current_col {
        d.columns.push(c);
    }
    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_task(body: &str) -> KanbanTask {
    // [text]@{ key: value, ... } or just text
    let (text_part, attrs) = if let Some(at) = body.find("@{") {
        (&body[..at], Some(&body[at + 2..]))
    } else {
        (body, None)
    };
    let text = text_part
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let id = text.clone();
    let mut assigned = None;
    let mut priority = None;
    if let Some(a) = attrs {
        let a = a.trim_end_matches('}').trim();
        for kv in a.split(',') {
            let (k, v) = match kv.split_once(':') {
                Some(p) => p,
                None => continue,
            };
            let k = k.trim();
            let v = v.trim().trim_matches('\'').trim_matches('"').to_string();
            match k {
                "assigned" => assigned = Some(v),
                "priority" => priority = Some(v),
                _ => {}
            }
        }
    }
    KanbanTask {
        id,
        text,
        assigned,
        priority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let d = parse("kanban\n  Todo\n    Task A\n    [Task B]\n  Done\n    [Task C]\n").unwrap();
        assert_eq!(d.columns.len(), 2);
        assert_eq!(d.columns[0].label, "Todo");
        assert_eq!(d.columns[0].tasks.len(), 2);
        assert_eq!(d.columns[1].tasks[0].text, "Task C");
    }

    #[test]
    fn task_with_attrs() {
        let d = parse("kanban\n  Todo\n    [Task X]@{ assigned: 'Alice', priority: 'High' }\n")
            .unwrap();
        let t = &d.columns[0].tasks[0];
        assert_eq!(t.assigned.as_deref(), Some("Alice"));
        assert_eq!(t.priority.as_deref(), Some("High"));
    }
}
