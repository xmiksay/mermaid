//! Per-line statement parsing: messages (with the `->>+`/`-->>-` activation
//! shorthand), participants, notes, autonumber, and actor menus.

use crate::parse::ast::{
    ArrowKind, AutoNumberConfig, Message, NotePosition, Participant, ParticipantKind,
    SequenceDiagram, SequenceItem, SequenceNote,
};
use crate::parse::ParseError;

const ARROWS: &[(&str, ArrowKind)] = &[
    ("<<-->>", ArrowKind::BiDashedArrow),
    ("<<->>", ArrowKind::BiSolidArrow),
    ("-->>", ArrowKind::DashedArrow),
    ("-->", ArrowKind::Dashed),
    ("--x", ArrowKind::DashedCross),
    ("--)", ArrowKind::DashedOpen),
    ("->>", ArrowKind::SolidArrow),
    ("->", ArrowKind::Solid),
    ("-x", ArrowKind::Cross),
    ("-)", ArrowKind::Open),
];

pub(super) fn parse_line_to_items(
    line: &str,
    diag: &mut SequenceDiagram,
    line_no: usize,
) -> Result<Vec<SequenceItem>, ParseError> {
    if let Some(rest) = line.strip_prefix("title ") {
        diag.title = Some(rest.trim().to_string());
        return Ok(Vec::new());
    }
    if line == "autonumber" || line.starts_with("autonumber ") {
        let cfg = parse_autonumber(line);
        if cfg.is_some() {
            diag.autonumber = true;
        }
        return Ok(vec![SequenceItem::AutoNumber(cfg)]);
    }

    if let Some(rest) = line.strip_prefix("participant ") {
        let p = parse_participant(rest, ParticipantKind::Participant, line_no)?;
        diag.participants.push(p);
        return Ok(Vec::new());
    }
    if let Some(rest) = line.strip_prefix("actor ") {
        let p = parse_participant(rest, ParticipantKind::Actor, line_no)?;
        diag.participants.push(p);
        return Ok(Vec::new());
    }

    if let Some(rest) = line.strip_prefix("create ") {
        let (kind, decl) = match (
            rest.trim().strip_prefix("participant "),
            rest.trim().strip_prefix("actor "),
        ) {
            (Some(r), _) => (ParticipantKind::Participant, r),
            (_, Some(r)) => (ParticipantKind::Actor, r),
            _ => (ParticipantKind::Participant, rest.trim()),
        };
        let p = parse_participant(decl, kind, line_no)?;
        let id = p.id.clone();
        diag.participants.push(p);
        return Ok(vec![SequenceItem::Create(id)]);
    }
    if let Some(rest) = line.strip_prefix("destroy ") {
        return Ok(vec![SequenceItem::Destroy(rest.trim().to_string())]);
    }

    // Actor menus (`link A: Label @ url`, `links A: {json}`) are consumed but
    // not rendered — accepting the syntax keeps them from being hard errors.
    if is_actor_menu(line) {
        return Ok(Vec::new());
    }

    if let Some(rest) = line.strip_prefix("activate ") {
        return Ok(vec![SequenceItem::Activate(rest.trim().to_string())]);
    }
    if let Some(rest) = line.strip_prefix("deactivate ") {
        return Ok(vec![SequenceItem::Deactivate(rest.trim().to_string())]);
    }

    if let Some(note) = parse_note(line) {
        return Ok(vec![SequenceItem::Note(note)]);
    }

    let (msg, activation) = parse_message(line, line_no)?;
    register_implicit_participant(diag, &msg.from);
    register_implicit_participant(diag, &msg.to);
    // Activation shorthand: `->>+B` activates the target *after* the message,
    // `-->>-A` deactivates it *before* the message — matching upstream ordering.
    let target = msg.to.clone();
    let mut items = Vec::new();
    match activation {
        Activation::Activate => {
            items.push(SequenceItem::Message(msg));
            items.push(SequenceItem::Activate(target));
        }
        Activation::Deactivate => {
            items.push(SequenceItem::Deactivate(target));
            items.push(SequenceItem::Message(msg));
        }
        Activation::None => items.push(SequenceItem::Message(msg)),
    }
    Ok(items)
}

/// `autonumber` / `autonumber <start>` / `autonumber <start> <step>` /
/// `autonumber off`. Bare `autonumber` starts at 1 with step 1; `off` returns
/// `None`. Non-numeric params fall back to the defaults.
fn parse_autonumber(line: &str) -> Option<AutoNumberConfig> {
    let rest = line["autonumber".len()..].trim();
    if rest.eq_ignore_ascii_case("off") {
        return None;
    }
    let mut nums = rest.split_whitespace();
    Some(AutoNumberConfig {
        start: nums.next().and_then(|s| s.parse().ok()).unwrap_or(1),
        step: nums.next().and_then(|s| s.parse().ok()).unwrap_or(1),
    })
}

fn parse_note(line: &str) -> Option<SequenceNote> {
    // Forms:
    //   Note over A[,B]: text
    //   Note right of A: text
    //   Note left of A: text
    let body = line
        .strip_prefix("Note ")
        .or_else(|| line.strip_prefix("note "))?;
    let (head, text) = body.split_once(':')?;
    let head = head.trim();
    let text = text.trim().to_string();
    let (position, target_part) = if let Some(rest) = head.strip_prefix("over ") {
        (NotePosition::Over, rest)
    } else if let Some(rest) = head.strip_prefix("right of ") {
        (NotePosition::RightOf, rest)
    } else if let Some(rest) = head.strip_prefix("left of ") {
        (NotePosition::LeftOf, rest)
    } else {
        return None;
    };
    let participants: Vec<String> = target_part
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(SequenceNote {
        position,
        participants,
        text,
    })
}

/// `link <id>: <label> @ <url>` and `links <id>: {json}` declare actor popup
/// menus. Recognize the shape (a `link`/`links` keyword, an id, then a colon)
/// so the line is consumed rather than mistaken for a message.
fn is_actor_menu(line: &str) -> bool {
    let rest = line
        .strip_prefix("links ")
        .or_else(|| line.strip_prefix("link "));
    match rest {
        Some(rest) => rest.split_once(':').is_some_and(|(id, _)| {
            let id = id.trim();
            !id.is_empty() && !id.contains(char::is_whitespace)
        }),
        None => false,
    }
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

/// Activation shorthand attached to a message arrow (`->>+` / `-->>-`).
enum Activation {
    None,
    Activate,
    Deactivate,
}

fn parse_message(line: &str, line_no: usize) -> Result<(Message, Activation), ParseError> {
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
    let (target_part, text) = match after.find(':') {
        Some(c) => (after[..c].trim(), after[c + 1..].trim().to_string()),
        None => (after.trim(), String::new()),
    };
    // A leading `+`/`-` on the target is the activation shorthand, not part of
    // the participant id.
    let (activation, target_part) = match target_part.strip_prefix('+') {
        Some(rest) => (Activation::Activate, rest.trim_start()),
        None => match target_part.strip_prefix('-') {
            Some(rest) => (Activation::Deactivate, rest.trim_start()),
            None => (Activation::None, target_part),
        },
    };
    let to = target_part.to_string();
    if to.is_empty() {
        return Err(ParseError::Syntax {
            message: "empty receiver".into(),
            line: line_no,
        });
    }
    Ok((
        Message {
            from,
            to,
            text,
            arrow: kind,
        },
        activation,
    ))
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
    use super::super::parse;
    use super::*;

    fn first_msg(d: &SequenceDiagram) -> &Message {
        for it in &d.items {
            if let SequenceItem::Message(m) = it {
                return m;
            }
        }
        panic!("no message");
    }

    #[test]
    fn explicit_participants_and_message() {
        let s = "sequenceDiagram\nparticipant alice\nparticipant bob as Bob\nalice->>bob: hi\n";
        let d = parse(s).unwrap();
        assert_eq!(d.participants.len(), 2);
        assert_eq!(first_msg(&d).arrow, ArrowKind::SolidArrow);
    }

    #[test]
    fn implicit_participants_from_messages() {
        let d = parse("sequenceDiagram\nA->B: ping\nB-->A: pong\n").unwrap();
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.items.len(), 2);
    }

    #[test]
    fn all_arrow_kinds_recognized() {
        let cases = [
            ("A->B: t", ArrowKind::Solid),
            ("A->>B: t", ArrowKind::SolidArrow),
            ("A-->B: t", ArrowKind::Dashed),
            ("A-->>B: t", ArrowKind::DashedArrow),
            ("A-xB: t", ArrowKind::Cross),
            ("A--xB: t", ArrowKind::DashedCross),
            ("A-)B: t", ArrowKind::Open),
            ("A--)B: t", ArrowKind::DashedOpen),
            ("A<<->>B: t", ArrowKind::BiSolidArrow),
            ("A<<-->>B: t", ArrowKind::BiDashedArrow),
        ];
        for (msg, expected) in cases {
            let s = format!("sequenceDiagram\n{msg}\n");
            let d = parse(&s).unwrap();
            assert_eq!(first_msg(&d).arrow, expected, "case: {msg}");
        }
    }

    #[test]
    fn bidirectional_arrow_no_phantom_participant() {
        // `Alice<<->>Bob` is one message between two participants, not a
        // phantom `Alice<<` participant (issue #57).
        let d = parse("sequenceDiagram\nAlice<<->>Bob: hi\n").unwrap();
        assert_eq!(d.participants.len(), 2);
        let m = first_msg(&d);
        assert_eq!(m.from, "Alice");
        assert_eq!(m.to, "Bob");
        assert_eq!(m.arrow, ArrowKind::BiSolidArrow);
    }

    #[test]
    fn notes() {
        let d = parse("sequenceDiagram\nA->>B: hi\nNote over A,B: shared\nNote right of B: thx\n")
            .unwrap();
        let notes: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| {
                if let SequenceItem::Note(n) = i {
                    Some(n)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].position, NotePosition::Over);
        assert_eq!(notes[0].participants.len(), 2);
        assert_eq!(notes[1].position, NotePosition::RightOf);
    }

    #[test]
    fn activate_deactivate() {
        let d = parse("sequenceDiagram\nactivate A\nA->>B: x\ndeactivate A\n").unwrap();
        assert!(matches!(d.items.first(), Some(SequenceItem::Activate(s)) if s == "A"));
        assert!(matches!(d.items.last(), Some(SequenceItem::Deactivate(s)) if s == "A"));
    }

    #[test]
    fn activation_shorthand_targets_and_events() {
        let d = parse("sequenceDiagram\nA->>+B: outer\nB-->>-A: r1\n").unwrap();
        // Only A and B — no bogus `+B` / `-A` participants.
        assert_eq!(d.participants.len(), 2);
        assert!(d.participants.iter().all(|p| p.id == "A" || p.id == "B"));

        // Sequence: Message(A->B), Activate(B), Deactivate(A), Message(B->A).
        assert!(matches!(&d.items[0], SequenceItem::Message(m) if m.to == "B"));
        assert!(matches!(&d.items[1], SequenceItem::Activate(s) if s == "B"));
        assert!(matches!(&d.items[2], SequenceItem::Deactivate(s) if s == "A"));
        assert!(matches!(&d.items[3], SequenceItem::Message(m) if m.to == "A"));
        assert_eq!(d.items.len(), 4);
    }

    #[test]
    fn activation_shorthand_preserves_text_and_arrow() {
        let d = parse("sequenceDiagram\nA->>+B: hello\n").unwrap();
        let m = first_msg(&d);
        assert_eq!(m.to, "B");
        assert_eq!(m.text, "hello");
        assert_eq!(m.arrow, ArrowKind::SolidArrow);
    }

    #[test]
    fn autonumber_sets_flag() {
        let d = parse("sequenceDiagram\nautonumber\nA->>B: x\n").unwrap();
        assert!(d.autonumber);
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::AutoNumber(Some(c))) if c.start == 1 && c.step == 1
        ));
    }

    #[test]
    fn autonumber_start_and_step() {
        let d = parse("sequenceDiagram\nautonumber 10 5\nA->>B: x\n").unwrap();
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::AutoNumber(Some(c))) if c.start == 10 && c.step == 5
        ));
    }

    #[test]
    fn autonumber_off_emits_none() {
        let d = parse("sequenceDiagram\nautonumber\nA->>B: x\nautonumber off\n").unwrap();
        // The trailing `off` is a positional None marker; it doesn't clear the
        // "was ever on" flag.
        assert!(d.autonumber);
        assert!(matches!(
            d.items.last(),
            Some(SequenceItem::AutoNumber(None))
        ));
    }

    #[test]
    fn title_and_actor() {
        let d = parse("sequenceDiagram\ntitle Login\nactor u as User\nu->>X: hi\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("Login"));
        assert_eq!(d.participants[0].kind, ParticipantKind::Actor);
    }

    #[test]
    fn create_registers_participant_and_emits_item() {
        let d = parse(
            "sequenceDiagram\nAlice->>Bob: Hello\ncreate participant Carl\nAlice->>Carl: Hi Carl\n",
        )
        .unwrap();
        // Carl is registered as a real participant (a column).
        assert!(d.participants.iter().any(|p| p.id == "Carl"));
        // A positional Create item sits before the creating message.
        let create_pos = d
            .items
            .iter()
            .position(|i| matches!(i, SequenceItem::Create(id) if id == "Carl"))
            .expect("no Create item");
        let msg_pos = d
            .items
            .iter()
            .position(|i| matches!(i, SequenceItem::Message(m) if m.to == "Carl"))
            .unwrap();
        assert!(create_pos < msg_pos);
    }

    #[test]
    fn create_actor_kind() {
        let d = parse("sequenceDiagram\ncreate actor D as Donald\nA->>D: hi\n").unwrap();
        let carl = d.participants.iter().find(|p| p.id == "D").unwrap();
        assert_eq!(carl.kind, ParticipantKind::Actor);
        assert_eq!(carl.display, "Donald");
    }

    #[test]
    fn actor_menu_lines_are_consumed() {
        // `link`/`links` actor menus must not be hard parse errors.
        let d = parse(
            "sequenceDiagram\nA->>B: hi\nlink A: Dashboard @ https://example.com\nlinks B: {\"Repo\": \"https://ex/repo\"}\n",
        )
        .unwrap();
        // Only the single message survives; the menu lines are dropped.
        assert_eq!(
            d.items
                .iter()
                .filter(|i| matches!(i, SequenceItem::Message(_)))
                .count(),
            1
        );
    }
}
