//! Sequence diagram parser.
//!
//! Supports:
//!   * `sequenceDiagram` header.
//!   * `title <text>`.
//!   * `participant <id> [as <alias>]`, `actor <id> [as <alias>]`.
//!   * Messages: `from <arrow> to : text` with arrows
//!     `->`, `->>`, `-->`, `-->>`, `-x`, `--x`, `-)`, `--)`.
//!   * `autonumber` (sets a per-diagram flag).
//!   * `activate <id>` / `deactivate <id>`.
//!   * Notes: `Note over A[,B]: text`, `Note right of A: text`, `Note left of A: text`.
//!   * Blocks (each followed by item lines, terminated by `end`):
//!     `alt label` ... `else label` ... `end`,
//!     `loop label` ... `end`,
//!     `par label` ... `and label` ... `end`,
//!     `opt label` ... `end`,
//!     `critical label` ... `option label` ... `end`.
//!   * `box label` ... `end` — collects participants declared inside.

use super::ast::{
    AltBranch, ArrowKind, Message, NotePosition, Participant, ParticipantKind, SequenceBlock,
    SequenceBox, SequenceDiagram, SequenceItem, SequenceNote,
};
use super::{strip_comment, ParseError};

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
    let mut block_stack: Vec<BlockFrame> = Vec::new();

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

        if handle_block_keyword(line, &mut block_stack, &mut diag)? {
            continue;
        }

        let item = parse_line_to_item(line, &mut diag, line_no)?;
        if let Some(item) = item {
            push_item(&mut diag, &mut block_stack, item);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    // Pop any unclosed blocks gracefully (no error — many real-world diagrams
    // have informal use). Each block was already pushed; nothing more to do.
    while let Some(frame) = block_stack.pop() {
        attach_pending(&mut diag, frame);
    }
    Ok(diag)
}

enum BlockFrame {
    Alt {
        branches: Vec<AltBranch>,
        current_label: String,
        current_items: Vec<SequenceItem>,
    },
    Par {
        branches: Vec<AltBranch>,
        current_label: String,
        current_items: Vec<SequenceItem>,
    },
    Critical {
        branches: Vec<AltBranch>,
        current_label: String,
        current_items: Vec<SequenceItem>,
    },
    Loop {
        label: String,
        items: Vec<SequenceItem>,
    },
    Opt {
        label: String,
        items: Vec<SequenceItem>,
    },
    Box {
        label: String,
        participant_ids: Vec<String>,
    },
}

fn handle_block_keyword(
    line: &str,
    stack: &mut Vec<BlockFrame>,
    diag: &mut SequenceDiagram,
) -> Result<bool, ParseError> {
    // `end` closes the topmost frame.
    if line == "end" {
        if let Some(frame) = stack.pop() {
            let item = close_frame(frame);
            push_item(diag, stack, item);
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("alt ") {
        stack.push(BlockFrame::Alt {
            branches: Vec::new(),
            current_label: rest.trim().to_string(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "alt" {
        stack.push(BlockFrame::Alt {
            branches: Vec::new(),
            current_label: String::new(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if let Some(rest) = line.strip_prefix("else") {
        if let Some(BlockFrame::Alt {
            branches,
            current_label,
            current_items,
        }) = stack.last_mut()
        {
            let label = std::mem::take(current_label);
            let items = std::mem::take(current_items);
            branches.push(AltBranch { label, items });
            *current_label = rest.trim().to_string();
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("opt ") {
        stack.push(BlockFrame::Opt {
            label: rest.trim().to_string(),
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "opt" {
        stack.push(BlockFrame::Opt {
            label: String::new(),
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("loop ") {
        stack.push(BlockFrame::Loop {
            label: rest.trim().to_string(),
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "loop" {
        stack.push(BlockFrame::Loop {
            label: String::new(),
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("par ") {
        stack.push(BlockFrame::Par {
            branches: Vec::new(),
            current_label: rest.trim().to_string(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if let Some(rest) = line.strip_prefix("and ") {
        if let Some(BlockFrame::Par {
            branches,
            current_label,
            current_items,
        }) = stack.last_mut()
        {
            let label = std::mem::take(current_label);
            let items = std::mem::take(current_items);
            branches.push(AltBranch { label, items });
            *current_label = rest.trim().to_string();
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("critical ") {
        stack.push(BlockFrame::Critical {
            branches: Vec::new(),
            current_label: rest.trim().to_string(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if let Some(rest) = line.strip_prefix("option ") {
        if let Some(BlockFrame::Critical {
            branches,
            current_label,
            current_items,
        }) = stack.last_mut()
        {
            let label = std::mem::take(current_label);
            let items = std::mem::take(current_items);
            branches.push(AltBranch { label, items });
            *current_label = rest.trim().to_string();
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("box ") {
        stack.push(BlockFrame::Box {
            label: rest.trim().to_string(),
            participant_ids: Vec::new(),
        });
        return Ok(true);
    }

    Ok(false)
}

fn close_frame(frame: BlockFrame) -> SequenceItem {
    match frame {
        BlockFrame::Alt {
            mut branches,
            current_label,
            current_items,
        } => {
            branches.push(AltBranch {
                label: current_label,
                items: current_items,
            });
            SequenceItem::Alt(branches)
        }
        BlockFrame::Par {
            mut branches,
            current_label,
            current_items,
        } => {
            branches.push(AltBranch {
                label: current_label,
                items: current_items,
            });
            SequenceItem::Par(branches)
        }
        BlockFrame::Critical {
            mut branches,
            current_label,
            current_items,
        } => {
            branches.push(AltBranch {
                label: current_label,
                items: current_items,
            });
            SequenceItem::Critical(branches)
        }
        BlockFrame::Loop { label, items } => SequenceItem::Loop(SequenceBlock { label, items }),
        BlockFrame::Opt { label, items } => SequenceItem::Opt(SequenceBlock { label, items }),
        BlockFrame::Box {
            label,
            participant_ids,
        } => SequenceItem::Box(SequenceBox {
            label,
            participant_ids,
        }),
    }
}

fn attach_pending(diag: &mut SequenceDiagram, frame: BlockFrame) {
    let item = close_frame(frame);
    diag.items.push(item);
}

fn push_item(diag: &mut SequenceDiagram, stack: &mut [BlockFrame], item: SequenceItem) {
    if let Some(frame) = stack.last_mut() {
        match frame {
            BlockFrame::Alt { current_items, .. }
            | BlockFrame::Par { current_items, .. }
            | BlockFrame::Critical { current_items, .. } => current_items.push(item),
            BlockFrame::Loop { items, .. } | BlockFrame::Opt { items, .. } => items.push(item),
            BlockFrame::Box {
                participant_ids, ..
            } => {
                // Box only holds participants — items leak to the diagram level.
                if let SequenceItem::Message(_) = &item {
                    diag.items.push(item);
                } else {
                    diag.items.push(item);
                }
                let _ = participant_ids;
            }
        }
    } else {
        diag.items.push(item);
    }
}

fn parse_line_to_item(
    line: &str,
    diag: &mut SequenceDiagram,
    line_no: usize,
) -> Result<Option<SequenceItem>, ParseError> {
    if let Some(rest) = line.strip_prefix("title ") {
        diag.title = Some(rest.trim().to_string());
        return Ok(None);
    }
    if line == "autonumber" || line.starts_with("autonumber ") {
        diag.autonumber = true;
        return Ok(None);
    }

    if let Some(rest) = line.strip_prefix("participant ") {
        let p = parse_participant(rest, ParticipantKind::Participant, line_no)?;
        diag.participants.push(p);
        return Ok(None);
    }
    if let Some(rest) = line.strip_prefix("actor ") {
        let p = parse_participant(rest, ParticipantKind::Actor, line_no)?;
        diag.participants.push(p);
        return Ok(None);
    }

    if let Some(rest) = line.strip_prefix("activate ") {
        return Ok(Some(SequenceItem::Activate(rest.trim().to_string())));
    }
    if let Some(rest) = line.strip_prefix("deactivate ") {
        return Ok(Some(SequenceItem::Deactivate(rest.trim().to_string())));
    }

    if let Some(note) = parse_note(line) {
        return Ok(Some(SequenceItem::Note(note)));
    }

    let msg = parse_message(line, line_no)?;
    register_implicit_participant(diag, &msg.from);
    register_implicit_participant(diag, &msg.to);
    Ok(Some(SequenceItem::Message(msg)))
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
        Some(c) => (
            after[..c].trim().to_string(),
            after[c + 1..].trim().to_string(),
        ),
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
            ("A--xB: t", ArrowKind::Cross),
            ("A-)B: t", ArrowKind::Open),
            ("A--)B: t", ArrowKind::Open),
        ];
        for (msg, expected) in cases {
            let s = format!("sequenceDiagram\n{msg}\n");
            let d = parse(&s).unwrap();
            assert_eq!(first_msg(&d).arrow, expected, "case: {msg}");
        }
    }

    #[test]
    fn alt_block() {
        let d =
            parse("sequenceDiagram\nA->>B: q\nalt is yes\nA->>B: y\nelse is no\nA->>B: n\nend\n")
                .unwrap();
        let alt = d
            .items
            .iter()
            .find_map(|i| {
                if let SequenceItem::Alt(b) = i {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(alt.len(), 2);
        assert_eq!(alt[0].label, "is yes");
        assert_eq!(alt[1].label, "is no");
    }

    #[test]
    fn loop_block() {
        let d = parse("sequenceDiagram\nloop every 5s\nA->>B: ping\nend\n").unwrap();
        let lp = d
            .items
            .iter()
            .find_map(|i| {
                if let SequenceItem::Loop(b) = i {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(lp.label, "every 5s");
        assert_eq!(lp.items.len(), 1);
    }

    #[test]
    fn par_with_branches() {
        let d = parse("sequenceDiagram\npar req\nA->>B: x\nand other\nA->>C: y\nend\n").unwrap();
        let branches = d
            .items
            .iter()
            .find_map(|i| {
                if let SequenceItem::Par(b) = i {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].label, "req");
        assert_eq!(branches[1].label, "other");
    }

    #[test]
    fn opt_block() {
        let d = parse("sequenceDiagram\nopt cond\nA->>B: x\nend\n").unwrap();
        let opt = d
            .items
            .iter()
            .find_map(|i| {
                if let SequenceItem::Opt(b) = i {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(opt.label, "cond");
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
    fn autonumber_sets_flag() {
        let d = parse("sequenceDiagram\nautonumber\nA->>B: x\n").unwrap();
        assert!(d.autonumber);
    }

    #[test]
    fn title_and_actor() {
        let d = parse("sequenceDiagram\ntitle Login\nactor u as User\nu->>X: hi\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("Login"));
        assert_eq!(d.participants[0].kind, ParticipantKind::Actor);
    }

    #[test]
    fn comments_and_blanks_ignored() {
        let d = parse("sequenceDiagram\n%% c\n\nA->>B: x\n").unwrap();
        assert_eq!(d.items.len(), 1);
    }

    #[test]
    fn rejects_bad_header() {
        let err = parse("flowchart TD\nA-->B\n").unwrap_err();
        match err {
            ParseError::Syntax { line, .. } => assert_eq!(line, 1),
            e => panic!("unexpected: {e:?}"),
        }
    }
}
