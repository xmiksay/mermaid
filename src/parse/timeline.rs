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
//! period `2002` with two events. A continuation line starting with `:` (e.g.
//! `: Google`) appends its events to the most recent period.

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
            d.direction = parse_header(line, line_no)?;
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
        } else if let Some(rest) = line.strip_prefix(':') {
            let events = parse_events(rest);
            if events.is_empty() {
                return Err(ParseError::malformed(
                    line_no,
                    "continuation line has no events",
                ));
            }
            let periods = match &mut current {
                Some(s) => &mut s.periods,
                None => &mut implicit.periods,
            };
            match periods.last_mut() {
                Some(period) => period.events.extend(events),
                None => {
                    return Err(ParseError::malformed(
                        line_no,
                        "continuation line before any period",
                    ));
                }
            }
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

/// Parse the `timeline` header, optionally followed by a v11.14+ direction
/// token (`TB`/`TD`/`BT`/`RL`/`LR`). Returns the validated direction if present.
fn parse_header(line: &str, line_no: usize) -> Result<Option<String>, ParseError> {
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("timeline") {
        return Err(ParseError::header(line_no, "expected 'timeline' header"));
    }
    let direction = match tokens.next() {
        None => None,
        Some(dir) => match dir {
            "TB" | "TD" | "BT" | "RL" | "LR" => Some(dir.to_string()),
            other => {
                return Err(ParseError::header(
                    line_no,
                    format!("unknown timeline direction '{other}'"),
                ));
            }
        },
    };
    if tokens.next().is_some() {
        return Err(ParseError::header(line_no, "trailing tokens after header"));
    }
    Ok(direction)
}

fn parse_period(line: &str, line_no: usize) -> Result<TimelinePeriod, ParseError> {
    let (label, rest) = match line.split_once(':') {
        Some((label, rest)) => (label.trim().to_string(), rest),
        None => (line.trim().to_string(), ""),
    };
    if label.is_empty() {
        return Err(ParseError::malformed(line_no, "empty period label"));
    }
    let events = parse_events(rest);
    if events.is_empty() {
        return Err(ParseError::malformed(
            line_no,
            format!("period '{label}' has no events"),
        ));
    }
    Ok(TimelinePeriod { label, events })
}

fn parse_events(rest: &str) -> Vec<String> {
    rest.split(':')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
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
    fn continuation_events() {
        let d = parse("timeline\n2004 : Facebook\n     : Google\n2005 : Youtube\n").unwrap();
        assert_eq!(d.sections[0].periods.len(), 2);
        assert_eq!(
            d.sections[0].periods[0].events,
            vec!["Facebook".to_string(), "Google".to_string()]
        );
        assert_eq!(d.sections[0].periods[1].label, "2005");
    }

    #[test]
    fn continuation_multi_event() {
        let d = parse("timeline\n2004 : Facebook\n : Google : Twitter\n").unwrap();
        assert_eq!(
            d.sections[0].periods[0].events,
            vec![
                "Facebook".to_string(),
                "Google".to_string(),
                "Twitter".to_string()
            ]
        );
    }

    #[test]
    fn continuation_within_section() {
        let d = parse("timeline\nsection S\n2004 : Facebook\n : Google\n").unwrap();
        assert_eq!(d.sections[0].name.as_deref(), Some("S"));
        assert_eq!(d.sections[0].periods[0].events.len(), 2);
    }

    #[test]
    fn continuation_before_period_errors() {
        assert!(matches!(
            parse("timeline\n : Google\n"),
            Err(ParseError::Syntax { .. })
        ));
    }

    #[test]
    fn requires_header() {
        assert!(matches!(parse("nope\n"), Err(ParseError::Syntax { .. })));
    }

    #[test]
    fn header_direction() {
        let d = parse("timeline LR\n2002 : LinkedIn\n").unwrap();
        assert_eq!(d.direction.as_deref(), Some("LR"));
        let d = parse("timeline TD\n2002 : LinkedIn\n").unwrap();
        assert_eq!(d.direction.as_deref(), Some("TD"));
        let d = parse("timeline\n2002 : LinkedIn\n").unwrap();
        assert_eq!(d.direction, None);
    }

    #[test]
    fn header_unknown_direction_errors() {
        assert!(matches!(
            parse("timeline sideways\n2002 : A\n"),
            Err(ParseError::Syntax { .. })
        ));
    }
}
