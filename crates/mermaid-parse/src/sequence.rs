//! Sequence diagram parser (subset for v0.1).
//!
//! Supports:
//!   * `sequenceDiagram` header
//!   * `title <text>`
//!   * `participant <id> [as <alias>]`
//!   * `actor <id> [as <alias>]`
//!   * messages: `<from> <arrow> <to> : <text>`
//!     with arrows `->`, `->>`, `-->`, `-->>`, `-x`, `--x`, `-)`, `--)`
//!
//! Not yet: alt/loop/par/opt blocks, notes, activate/deactivate, autonumber.

use crate::ast::{ArrowKind, Message, Participant, ParticipantKind, SequenceDiagram};
use crate::{strip_comment, ParseError};

const ARROWS: &[(&str, ArrowKind)] = &[
    ("-->>", ArrowKind::DashedArrow),
    ("-->", ArrowKind::Dashed),
    ("--x", ArrowKind::Cross),
    ("--)", ArrowKind::Open),
    ("->>", ArrowKind::SolidArrow),
    ("->", ArrowKind::Solid),
    ("-x", ArrowKind::Cross),
    ("-)", ArrowKind::Open),
];

pub(crate) fn parse(input: &str) -> Result<SequenceDiagram, ParseError> {
    let mut diag = SequenceDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "sequenceDiagram" {
                return Err(ParseError::Syntax {
                    message: "expected 'sequenceDiagram' header".into(),
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
        if line == "title" {
            return Err(ParseError::Syntax {
                message: "empty title".into(),
                line: line_no,
            });
        }

        if let Some(rest) = line.strip_prefix("participant ") {
            diag.participants
                .push(parse_participant(rest, ParticipantKind::Participant, line_no)?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("actor ") {
            diag.participants
                .push(parse_participant(rest, ParticipantKind::Actor, line_no)?);
            continue;
        }

        let msg = parse_message(line, line_no)?;
        register_implicit_participant(&mut diag, &msg.from);
        register_implicit_participant(&mut diag, &msg.to);
        diag.messages.push(msg);
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

fn parse_participant(
    s: &str,
    kind: ParticipantKind,
    line_no: usize,
) -> Result<Participant, ParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ParseError::Syntax {
            message: "missing participant id".into(),
            line: line_no,
        });
    }
    if let Some((id, alias)) = s.split_once(" as ") {
        Ok(Participant {
            id: id.trim().to_string(),
            display: alias.trim().to_string(),
            kind,
        })
    } else {
        Ok(Participant {
            id: s.to_string(),
            display: s.to_string(),
            kind,
        })
    }
}

fn parse_message(line: &str, line_no: usize) -> Result<Message, ParseError> {
    let (arrow_pos, token, kind) = find_arrow(line).ok_or_else(|| ParseError::Syntax {
        message: format!("not a recognized statement: '{line}'"),
        line: line_no,
    })?;

    let from = line[..arrow_pos].trim().to_string();
    if from.is_empty() {
        return Err(ParseError::Syntax {
            message: "empty sender".into(),
            line: line_no,
        });
    }

    let after = &line[arrow_pos + token.len()..];
    let (to, text) = match after.find(':') {
        Some(c) => (after[..c].trim().to_string(), after[c + 1..].trim().to_string()),
        None => (after.trim().to_string(), String::new()),
    };
    if to.is_empty() {
        return Err(ParseError::Syntax {
            message: "empty receiver".into(),
            line: line_no,
        });
    }
    Ok(Message {
        from,
        to,
        text,
        arrow: kind,
    })
}

fn find_arrow(line: &str) -> Option<(usize, &'static str, ArrowKind)> {
    let mut best: Option<(usize, &'static str, ArrowKind)> = None;
    for &(tok, kind) in ARROWS {
        if let Some(pos) = line.find(tok) {
            match best {
                Some((p, _, _)) if p < pos => {}
                Some((p, t, _)) if p == pos && t.len() >= tok.len() => {}
                _ => best = Some((pos, tok, kind)),
            }
        }
    }
    best
}

fn register_implicit_participant(diag: &mut SequenceDiagram, id: &str) {
    if diag.participants.iter().any(|p| p.id == id) {
        return;
    }
    diag.participants.push(Participant {
        id: id.to_string(),
        display: id.to_string(),
        kind: ParticipantKind::Participant,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_participants_and_message() {
        let s = "sequenceDiagram\n\
             participant alice\n\
             participant bob as Bob\n\
             alice->>bob: hi\n";
        let d = parse(s).unwrap();
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.participants[0].id, "alice");
        assert_eq!(d.participants[1].display, "Bob");
        assert_eq!(d.messages.len(), 1);
        assert_eq!(d.messages[0].from, "alice");
        assert_eq!(d.messages[0].to, "bob");
        assert_eq!(d.messages[0].text, "hi");
        assert_eq!(d.messages[0].arrow, ArrowKind::SolidArrow);
    }

    #[test]
    fn implicit_participants_from_messages() {
        let s = "sequenceDiagram\nA->B: ping\nB-->A: pong\n";
        let d = parse(s).unwrap();
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.participants[0].id, "A");
        assert_eq!(d.participants[1].id, "B");
        assert_eq!(d.messages[0].arrow, ArrowKind::Solid);
        assert_eq!(d.messages[1].arrow, ArrowKind::Dashed);
    }

    #[test]
    fn all_arrow_kinds_recognized() {
        let cases = [
            ("A->B: t", ArrowKind::Solid),
            ("A->>B: t", ArrowKind::SolidArrow),
            ("A-->B: t", ArrowKind::Dashed),
            ("A-->>B: t", ArrowKind::DashedArrow),
            ("A-xB: t", ArrowKind::Cross),
            ("A--xB: t", ArrowKind::Cross),
            ("A-)B: t", ArrowKind::Open),
            ("A--)B: t", ArrowKind::Open),
        ];
        for (msg, expected) in cases {
            let s = format!("sequenceDiagram\n{msg}\n");
            let d = parse(&s).unwrap();
            assert_eq!(d.messages[0].arrow, expected, "case: {msg}");
        }
    }

    #[test]
    fn title_and_actor() {
        let s = "sequenceDiagram\ntitle Login flow\nactor user as User\nparticipant api\nuser->>api: login\n";
        let d = parse(s).unwrap();
        assert_eq!(d.title.as_deref(), Some("Login flow"));
        assert_eq!(d.participants[0].kind, ParticipantKind::Actor);
        assert_eq!(d.participants[0].display, "User");
        assert_eq!(d.participants[1].kind, ParticipantKind::Participant);
    }

    #[test]
    fn comments_and_blanks_ignored() {
        let s = "sequenceDiagram\n%% header comment\n\nA->>B: x\n%%trailing\n";
        let d = parse(s).unwrap();
        assert_eq!(d.messages.len(), 1);
    }

    #[test]
    fn rejects_bad_header() {
        let err = parse("flowchart TD\nA-->B\n").unwrap_err();
        match err {
            ParseError::Syntax { line, .. } => assert_eq!(line, 1),
            e => panic!("unexpected: {e:?}"),
        }
    }

    #[test]
    fn rejects_unrecognized_statement() {
        let err = parse("sequenceDiagram\nweird line here\n").unwrap_err();
        match err {
            ParseError::Syntax { line, message } => {
                assert_eq!(line, 2);
                assert!(message.contains("not a recognized"));
            }
            e => panic!("unexpected: {e:?}"),
        }
    }
}
