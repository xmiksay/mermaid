//! Kanban parser. Indentation-based:
//!
//! ```text
//! kanban
//!   todo[Todo]
//!     id1[Task 1]@{ assigned: 'Alice', priority: 'High', ticket: MC-2037 }
//!     Task 2
//!   doing[Doing]
//!     [Task 3]
//!   Done
//!     [Task 4]
//! ```
//!
//! Both columns and tasks accept the documented `id[Label]` bracket form: the
//! part before `[` is the id, the bracketed text the display label.

use super::ast::{KanbanColumn, KanbanDiagram, KanbanTask};
use super::token::split_unquoted;
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
                return Err(ParseError::header(line_no, "expected 'kanban' header"));
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
            let (id, label) = split_id_label(body);
            current_col = Some(KanbanColumn {
                id,
                label,
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
    // `id[Text]@{ key: value, ... }`, `[Text]`, or just `Text`.
    let (text_part, attrs) = if let Some(at) = body.find("@{") {
        (&body[..at], Some(&body[at + 2..]))
    } else {
        (body, None)
    };
    let (id, text) = split_id_label(text_part);
    let mut assigned = None;
    let mut priority = None;
    let mut ticket = None;
    if let Some(a) = attrs {
        let a = a.trim_end_matches('}').trim();
        for kv in split_unquoted(a, ',') {
            let (k, v) = match kv.split_once(':') {
                Some(p) => p,
                None => continue,
            };
            let k = k.trim();
            let v = v.trim().trim_matches('\'').trim_matches('"').to_string();
            match k {
                "assigned" => assigned = Some(v),
                "priority" => priority = Some(v),
                "ticket" => ticket = Some(v),
                _ => {}
            }
        }
    }
    KanbanTask {
        id,
        text,
        assigned,
        priority,
        ticket,
    }
}

/// Split the documented `id[Label]` form into `(id, label)`. Without brackets
/// the whole string is both id and label; a bracket form with an empty prefix
/// (`[Label]`) reuses the label as the id.
fn split_id_label(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(open) = s.find('[') {
        if s.ends_with(']') {
            let id = s[..open].trim();
            let label = s[open + 1..s.len() - 1].trim();
            let id = if id.is_empty() { label } else { id };
            return (id.to_string(), label.to_string());
        }
    }
    (s.to_string(), s.to_string())
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

    #[test]
    fn id_label_bracket_form() {
        let d = parse("kanban\n  todo[Todo]\n    id1[Create Documentation]\n").unwrap();
        assert_eq!(d.columns[0].id, "todo");
        assert_eq!(d.columns[0].label, "Todo");
        let t = &d.columns[0].tasks[0];
        assert_eq!(t.id, "id1");
        assert_eq!(t.text, "Create Documentation");
    }

    #[test]
    fn task_ticket_metadata() {
        let d = parse(
            "kanban\n  todo[Todo]\n    id2[Write blog]@{ ticket: MC-2037, priority: 'Very High' }\n",
        )
        .unwrap();
        let t = &d.columns[0].tasks[0];
        assert_eq!(t.id, "id2");
        assert_eq!(t.text, "Write blog");
        assert_eq!(t.ticket.as_deref(), Some("MC-2037"));
        assert_eq!(t.priority.as_deref(), Some("Very High"));
    }

    #[test]
    fn attrs_split_is_quote_aware() {
        let d = parse(
            "kanban\n  todo[Todo]\n    t1[Task]@{ assigned: 'Alice, Bob', priority: 'High' }\n",
        )
        .unwrap();
        let t = &d.columns[0].tasks[0];
        assert_eq!(t.assigned.as_deref(), Some("Alice, Bob"));
        assert_eq!(t.priority.as_deref(), Some("High"));
    }

    #[test]
    fn bare_bracket_task_reuses_label_as_id() {
        let d = parse("kanban\n  Todo\n    [Task B]\n").unwrap();
        let t = &d.columns[0].tasks[0];
        assert_eq!(t.id, "Task B");
        assert_eq!(t.text, "Task B");
    }

    #[test]
    fn ticket_base_url_from_frontmatter() {
        let src = "---\nconfig:\n  kanban:\n    ticketBaseUrl: 'https://example.com/#TICKET#'\n---\nkanban\n  todo[Todo]\n    id1[Task]@{ ticket: MC-1 }\n";
        let d = crate::parse::parse(src).unwrap();
        match d {
            crate::parse::Diagram::Kanban(k) => {
                assert_eq!(
                    k.ticket_base_url.as_deref(),
                    Some("https://example.com/#TICKET#")
                );
            }
            _ => panic!("expected kanban"),
        }
    }
}
