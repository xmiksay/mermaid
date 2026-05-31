//! ZenUML parser. ZenUML is a sequence-style notation; we translate it to a
//! [`SequenceDiagram`] so it reuses the sequence renderer.
//!
//! Supported subset (one statement per line):
//!
//! ```text
//! zenuml
//!     title <text>
//!     <From> -> <To>: <message>
//!     <From> ->> <To>: <message>
//!     <From>.<method>(<args>)         // implies From -> method-receiver
//! ```

use super::ast::{ArrowKind, Message, Participant, ParticipantKind, SequenceDiagram, SequenceItem};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<SequenceDiagram, ParseError> {
    let mut d = SequenceDiagram::default();
    let mut header_seen = false;
    let mut seen_participants: Vec<String> = Vec::new();

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if !header_seen {
            if line != "zenuml" {
                return Err(ParseError::Syntax {
                    message: "expected 'zenuml' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
            continue;
        }

        // Recognise the arrow form: A -> B : msg / A ->> B : msg.
        let (arrow, sep) = if line.contains("->>") {
            (ArrowKind::SolidArrow, "->>")
        } else if line.contains("->") {
            (ArrowKind::Solid, "->")
        } else {
            // method-call form: A.b()
            if let Some(dot) = line.find('.') {
                let from = line[..dot].trim().to_string();
                let call = line[dot + 1..].trim();
                let to = call.split('(').next().unwrap_or("").trim().to_string();
                if from.is_empty() || to.is_empty() {
                    continue;
                }
                ensure(&mut seen_participants, &mut d, &from);
                ensure(&mut seen_participants, &mut d, &to);
                d.items.push(SequenceItem::Message(Message {
                    from,
                    to,
                    text: call.to_string(),
                    arrow: ArrowKind::SolidArrow,
                }));
                continue;
            }
            return Err(ParseError::Syntax {
                message: format!("unrecognised zenuml line: '{line}'"),
                line: line_no,
            });
        };

        let (left, rest) = line.split_once(sep).unwrap();
        let (right, text) = match rest.split_once(':') {
            Some((r, t)) => (r.trim(), t.trim().to_string()),
            None => (rest.trim(), String::new()),
        };
        let from = left.trim().to_string();
        let to = right.to_string();
        ensure(&mut seen_participants, &mut d, &from);
        ensure(&mut seen_participants, &mut d, &to);
        d.items.push(SequenceItem::Message(Message {
            from,
            to,
            text,
            arrow,
        }));
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn ensure(seen: &mut Vec<String>, d: &mut SequenceDiagram, id: &str) {
    if !seen.contains(&id.to_string()) {
        seen.push(id.to_string());
        d.participants.push(Participant {
            id: id.to_string(),
            display: id.to_string(),
            kind: ParticipantKind::Participant,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arrow() {
        let d = parse("zenuml\nAlice -> Bob: Hello\nBob ->> Alice: Reply\n").unwrap();
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.items.len(), 2);
    }

    #[test]
    fn method_call() {
        let d = parse("zenuml\nA.b()\n").unwrap();
        assert_eq!(d.participants.len(), 2);
    }
}
