//! Timeline parser.
//!
//! Grammar:
//!
//! ```text
//! timeline
//!     title <text>
//!     section <text>
//!         <period> : <event1> : <event2>
//!         <period> : <event>
//! ```
//!
//! Sections are optional. A line `2002 : LinkedIn : Facebook` produces a
//! period `2002` with two events.

use super::ast::{TimelineDiagram, TimelinePeriod, TimelineSection};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<TimelineDiagram, ParseError> {
    let mut d = TimelineDiagram::default();
    let mut header_seen = false;
    let mut current: Option<TimelineSection> = None;
    let mut implicit: TimelineSection = TimelineSection {
        name: None,
        periods: Vec::new(),
    };

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "timeline" {
                return Err(ParseError::Syntax {
                    message: "expected 'timeline' header".into(),
                    line: line_no,
                });
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
            current = Some(TimelineSection {
                name: Some(rest.trim().to_string()),
                periods: Vec::new(),
            });
        } else {
            let period = parse_period(line, line_no)?;
            match &mut current {
                Some(s) => s.periods.push(period),
                None => implicit.periods.push(period),
            }
        }
    }

    if let Some(sec) = current {
        d.sections.push(sec);
    }
    if !implicit.periods.is_empty() {
        d.sections.insert(0, implicit);
    }
    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_period(line: &str, line_no: usize) -> Result<TimelinePeriod, ParseError> {
    let mut parts = line.split(':');
    let label = parts
        .next()
        .ok_or_else(|| ParseError::Syntax {
            message: format!("expected '<period> : <event>': '{line}'"),
            line: line_no,
        })?
        .trim()
        .to_string();
    if label.is_empty() {
        return Err(ParseError::Syntax {
            message: "empty period label".into(),
            line: line_no,
        });
    }
    let events: Vec<String> = parts
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if events.is_empty() {
        return Err(ParseError::Syntax {
            message: format!("period '{label}' has no events"),
            line: line_no,
        });
    }
    Ok(TimelinePeriod { label, events })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let d = parse("timeline\n2002 : LinkedIn\n").unwrap();
        assert_eq!(d.sections.len(), 1);
        assert_eq!(d.sections[0].periods[0].label, "2002");
        assert_eq!(
            d.sections[0].periods[0].events,
            vec!["LinkedIn".to_string()]
        );
    }

    #[test]
    fn multi_event() {
        let d = parse("timeline\n2004 : Facebook : Google\n").unwrap();
        assert_eq!(d.sections[0].periods[0].events.len(), 2);
    }

    #[test]
    fn sections() {
        let d =
            parse("timeline\ntitle T\nsection 20s\n2002 : A\n2003 : B\nsection 21s\n2010 : C\n")
                .unwrap();
        assert_eq!(d.title.as_deref(), Some("T"));
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[0].periods.len(), 2);
    }

    #[test]
    fn requires_header() {
        assert!(matches!(parse("nope\n"), Err(ParseError::Syntax { .. })));
    }
}
