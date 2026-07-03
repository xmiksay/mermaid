//! User-journey parser.
//!
//! Grammar:
//!
//! ```text
//! journey
//!     title <text>
//!     section <text>
//!         Task name: <score>: Actor1, Actor2
//! ```

use super::ast::{JourneyDiagram, JourneySection, JourneyTask};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<JourneyDiagram, ParseError> {
    let mut d = JourneyDiagram::default();
    let mut header_seen = false;
    let mut current: Option<JourneySection> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "journey" {
                return Err(ParseError::header(line_no, "expected 'journey' header"));
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("section") {
            if let Some(sec) = current.take() {
                d.sections.push(sec);
            }
            current = Some(JourneySection {
                name: rest.trim().to_string(),
                tasks: Vec::new(),
            });
        } else {
            let task = parse_task(line, line_no)?;
            let sec = current.get_or_insert_with(|| JourneySection {
                name: String::new(),
                tasks: Vec::new(),
            });
            sec.tasks.push(task);
        }
    }

    if let Some(sec) = current {
        d.sections.push(sec);
    }
    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_task(line: &str, line_no: usize) -> Result<JourneyTask, ParseError> {
    let mut parts = line.splitn(3, ':');
    let name = parts
        .next()
        .ok_or_else(|| {
            ParseError::malformed(line_no, format!("expected 'name: score: actors': '{line}'"))
        })?
        .trim()
        .to_string();
    let score_str = parts
        .next()
        .ok_or_else(|| {
            ParseError::malformed(line_no, format!("expected score after first ':': '{line}'"))
        })?
        .trim();
    let score: i32 = score_str
        .parse()
        .map_err(|_| ParseError::number(line_no, format!("invalid score: '{score_str}'")))?;
    let actors = parts
        .next()
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Ok(JourneyTask {
        name,
        score,
        actors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let d = parse("journey\nsection Day\nWake: 3: Me\n").unwrap();
        assert_eq!(d.sections.len(), 1);
        assert_eq!(d.sections[0].name, "Day");
        assert_eq!(d.sections[0].tasks[0].name, "Wake");
        assert_eq!(d.sections[0].tasks[0].score, 3);
        assert_eq!(d.sections[0].tasks[0].actors, vec!["Me".to_string()]);
    }

    #[test]
    fn multiple_actors() {
        let d = parse("journey\nsection X\nDo work: 5: Alice, Bob, Carol\n").unwrap();
        assert_eq!(d.sections[0].tasks[0].actors.len(), 3);
    }

    #[test]
    fn title() {
        let d = parse("journey\ntitle My Day\nsection A\nT: 1: X\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("My Day"));
    }

    #[test]
    fn task_without_section_uses_implicit() {
        let d = parse("journey\nA: 1: U\n").unwrap();
        assert_eq!(d.sections.len(), 1);
        assert!(d.sections[0].name.is_empty());
    }

    #[test]
    fn requires_header() {
        assert!(matches!(parse("hello\n"), Err(ParseError::Syntax { .. })));
    }
}
