//! Pie chart parser.
//!
//! Grammar (line-based):
//!
//! ```text
//! pie [showData] [title <text>]
//! "label" : <number>
//! ...
//! ```
//!
//! Empty lines and `%%` comments are skipped.

use crate::ast::{PieDiagram, PieEntry};
use crate::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<PieDiagram, ParseError> {
    let mut pie = PieDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            parse_header(line, &mut pie, line_no)?;
            header_seen = true;
            continue;
        }

        let entry = parse_entry(line, line_no)?;
        pie.entries.push(entry);
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(pie)
}

fn parse_header(line: &str, pie: &mut PieDiagram, line_no: usize) -> Result<(), ParseError> {
    let rest = line.strip_prefix("pie").ok_or_else(|| ParseError::Syntax {
        message: "expected 'pie' header".into(),
        line: line_no,
    })?;
    let rest = rest.trim_start();

    let rest = if let Some(r) = rest.strip_prefix("showData") {
        pie.show_data = true;
        r.trim_start()
    } else {
        rest
    };

    if let Some(after_title) = rest.strip_prefix("title") {
        let title = after_title.trim();
        if title.is_empty() {
            return Err(ParseError::Syntax {
                message: "empty title after 'title'".into(),
                line: line_no,
            });
        }
        pie.title = Some(title.to_string());
    } else if !rest.is_empty() {
        return Err(ParseError::Syntax {
            message: format!("unexpected text after 'pie': '{rest}'"),
            line: line_no,
        });
    }
    Ok(())
}

fn parse_entry(line: &str, line_no: usize) -> Result<PieEntry, ParseError> {
    let (label_raw, value_raw) = line.rsplit_once(':').ok_or_else(|| ParseError::Syntax {
        message: format!("expected '<label> : <number>': '{line}'"),
        line: line_no,
    })?;
    let label_raw = label_raw.trim();
    let label = if label_raw.starts_with('"') && label_raw.ends_with('"') && label_raw.len() >= 2 {
        label_raw[1..label_raw.len() - 1].to_string()
    } else {
        label_raw.to_string()
    };
    if label.is_empty() {
        return Err(ParseError::Syntax {
            message: "empty label".into(),
            line: line_no,
        });
    }
    let value: f64 = value_raw.trim().parse().map_err(|_| ParseError::Syntax {
        message: format!("invalid number: '{}'", value_raw.trim()),
        line: line_no,
    })?;
    if value.is_nan() {
        return Err(ParseError::Syntax {
            message: "value is NaN".into(),
            line: line_no,
        });
    }
    Ok(PieEntry { label, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let p = parse("pie\n\"A\" : 10\n\"B\" : 20\n").unwrap();
        assert_eq!(p.title, None);
        assert!(!p.show_data);
        assert_eq!(p.entries.len(), 2);
        assert_eq!(p.entries[0].label, "A");
        assert_eq!(p.entries[0].value, 10.0);
        assert_eq!(p.entries[1].label, "B");
        assert_eq!(p.entries[1].value, 20.0);
    }

    #[test]
    fn with_title_and_show_data() {
        let p = parse("pie showData title My Chart\n\"X\" : 1.5\n").unwrap();
        assert_eq!(p.title.as_deref(), Some("My Chart"));
        assert!(p.show_data);
        assert_eq!(p.entries[0].value, 1.5);
    }

    #[test]
    fn unquoted_label() {
        let p = parse("pie\nFoo : 3\n").unwrap();
        assert_eq!(p.entries[0].label, "Foo");
    }

    #[test]
    fn skips_comments_and_blanks() {
        let p = parse("pie\n%% a comment\n\n\"A\" : 1\n%% trailing\n").unwrap();
        assert_eq!(p.entries.len(), 1);
    }

    #[test]
    fn rejects_bad_entry() {
        let err = parse("pie\nno colon here\n").unwrap_err();
        match err {
            ParseError::Syntax { line, .. } => assert_eq!(line, 2),
            e => panic!("unexpected: {e:?}"),
        }
    }

    #[test]
    fn rejects_bad_number() {
        let err = parse("pie\n\"A\" : not-a-number\n").unwrap_err();
        match err {
            ParseError::Syntax { line, message } => {
                assert_eq!(line, 2);
                assert!(message.contains("invalid number"));
            }
            e => panic!("unexpected: {e:?}"),
        }
    }

    #[test]
    fn missing_header_is_empty() {
        assert_eq!(parse("\n\n").unwrap_err(), ParseError::Empty);
    }
}
