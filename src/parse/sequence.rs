//! Sequence diagram parser.
//!
//! Supports:
//!   * `sequenceDiagram` header.
//!   * `title <text>`.
//!   * `participant <id> [as <alias>]`, `actor <id> [as <alias>]`.
//!   * Messages: `from <arrow> to : text` with arrows
//!     `->`, `->>`, `-->`, `-->>`, `-x`, `--x`, `-)`, `--)`, and the
//!     bidirectional forms `<<->>`, `<<-->>`.
//!   * `autonumber` (sets a per-diagram flag).
//!   * `activate <id>` / `deactivate <id>`, plus the `->>+`/`-->>-` arrow
//!     activation shorthand.
//!   * Notes: `Note over A[,B]: text`, `Note right of A: text`, `Note left of A: text`.
//!   * Blocks (each followed by item lines, terminated by `end`):
//!     `alt label` ... `else label` ... `end`,
//!     `loop label` ... `end`,
//!     `par label` ... `and label` ... `end`,
//!     `opt label` ... `end`,
//!     `critical label` ... `option label` ... `end`.
//!   * `box label` ... `end` — collects participants declared inside.

use super::ast::{
    AltBranch, ArrowKind, AutoNumberConfig, Message, NotePosition, Participant, ParticipantKind,
    SequenceBlock, SequenceBox, SequenceDiagram, SequenceItem, SequenceNote, SequenceRect,
};
use super::{strip_comment, ParseError};

const ARROWS: &[(&str, ArrowKind)] = &[
    ("<<-->>", ArrowKind::BiDashedArrow),
    ("<<->>", ArrowKind::BiSolidArrow),
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

        let before = diag.participants.len();
        let items = parse_line_to_items(line, &mut diag, line_no)?;
        // Participants (explicit or implied by a message) declared while a box
        // frame is open belong to that box.
        if diag.participants.len() > before {
            if let Some(BlockFrame::Box {
                participant_ids, ..
            }) = block_stack.last_mut()
            {
                for p in &diag.participants[before..] {
                    participant_ids.push(p.id.clone());
                }
            }
        }
        for item in items {
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
    Break {
        label: String,
        items: Vec<SequenceItem>,
    },
    Rect {
        color: Option<String>,
        items: Vec<SequenceItem>,
    },
    Box {
        color: Option<String>,
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

    if let Some(rest) = line.strip_prefix("break ") {
        stack.push(BlockFrame::Break {
            label: rest.trim().to_string(),
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "break" {
        stack.push(BlockFrame::Break {
            label: String::new(),
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("rect ") {
        // `rect <color>` — the whole argument is the fill (a bare label with no
        // color makes no sense for a background band).
        let arg = rest.trim();
        let color = (!arg.is_empty()).then(|| arg.to_string());
        stack.push(BlockFrame::Rect {
            color,
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "rect" {
        stack.push(BlockFrame::Rect {
            color: None,
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("box ") {
        let (color, label) = split_box_color(rest.trim());
        stack.push(BlockFrame::Box {
            color,
            label,
            participant_ids: Vec::new(),
        });
        return Ok(true);
    }
    if line == "box" {
        stack.push(BlockFrame::Box {
            color: None,
            label: String::new(),
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
        BlockFrame::Break { label, items } => SequenceItem::Break(SequenceBlock { label, items }),
        BlockFrame::Rect { color, items } => SequenceItem::Rect(SequenceRect { color, items }),
        BlockFrame::Box {
            color,
            label,
            participant_ids,
        } => SequenceItem::Box(SequenceBox {
            color,
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
            BlockFrame::Loop { items, .. }
            | BlockFrame::Opt { items, .. }
            | BlockFrame::Break { items, .. }
            | BlockFrame::Rect { items, .. } => items.push(item),
            // A box only groups participants; any messages/notes inside it are
            // ordinary events that belong at the diagram level.
            BlockFrame::Box { .. } => diag.items.push(item),
        }
    } else {
        diag.items.push(item);
    }
}

fn parse_line_to_items(
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

/// Split a `box <color> <label>` header into an optional leading color and the
/// remaining label. Mermaid treats the first token as a color when it is a hex
/// value, an `rgb(...)`/`rgba(...)` function, or a named CSS color; otherwise
/// the whole string is the label.
fn split_box_color(s: &str) -> (Option<String>, String) {
    if let Some(rest) = s.strip_prefix("rgb(").or_else(|| s.strip_prefix("rgba(")) {
        if let Some(close) = rest.find(')') {
            let end = s.len() - rest.len() + close + 1;
            let color = s[..end].to_string();
            let label = s[end..].trim().to_string();
            return (Some(color), label);
        }
    }
    let (first, rest) = match s.split_once(char::is_whitespace) {
        Some((a, b)) => (a, b.trim()),
        None => (s, ""),
    };
    if is_color_token(first) {
        (Some(first.to_string()), rest.to_string())
    } else {
        (None, s.to_string())
    }
}

fn is_color_token(tok: &str) -> bool {
    if tok.starts_with('#') {
        return true;
    }
    const NAMED: &[&str] = &[
        "transparent",
        "aqua",
        "black",
        "blue",
        "cyan",
        "fuchsia",
        "gray",
        "grey",
        "green",
        "lightblue",
        "lightgray",
        "lightgreen",
        "lightgrey",
        "lightyellow",
        "lime",
        "magenta",
        "maroon",
        "navy",
        "olive",
        "orange",
        "pink",
        "purple",
        "red",
        "silver",
        "teal",
        "white",
        "yellow",
    ];
    NAMED.contains(&tok.to_ascii_lowercase().as_str())
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
    fn break_block_parses() {
        let d = parse("sequenceDiagram\nbreak boom\nA->>B: x\nend\n").unwrap();
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::Break(b)) if b.label == "boom" && b.items.len() == 1
        ));
    }

    #[test]
    fn rect_block_captures_color_and_items() {
        let d = parse("sequenceDiagram\nrect rgb(0,255,0)\nA->>B: x\nend\n").unwrap();
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::Rect(r)) if r.color.as_deref() == Some("rgb(0,255,0)") && r.items.len() == 1
        ));
    }

    #[test]
    fn title_and_actor() {
        let d = parse("sequenceDiagram\ntitle Login\nactor u as User\nu->>X: hi\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("Login"));
        assert_eq!(d.participants[0].kind, ParticipantKind::Actor);
    }

    fn first_box(d: &SequenceDiagram) -> &SequenceBox {
        d.items
            .iter()
            .find_map(|i| match i {
                SequenceItem::Box(b) => Some(b),
                _ => None,
            })
            .expect("no box")
    }

    #[test]
    fn box_captures_members_and_color() {
        let d = parse(
            "sequenceDiagram\nbox Aqua Group\nparticipant A\nactor B\nend\nparticipant C\nA->>C: hi\n",
        )
        .unwrap();
        let b = first_box(&d);
        assert_eq!(b.color.as_deref(), Some("Aqua"));
        assert_eq!(b.label, "Group");
        assert_eq!(b.participant_ids, vec!["A".to_string(), "B".to_string()]);
        // C declared outside the box is not a member.
        assert!(!b.participant_ids.contains(&"C".to_string()));
    }

    #[test]
    fn box_without_color_keeps_full_label() {
        let d = parse("sequenceDiagram\nbox My Team\nparticipant A\nend\n").unwrap();
        let b = first_box(&d);
        assert_eq!(b.color, None);
        assert_eq!(b.label, "My Team");
    }

    #[test]
    fn box_rgb_color() {
        let d =
            parse("sequenceDiagram\nbox rgb(200, 200, 255) Team\nparticipant A\nend\n").unwrap();
        let b = first_box(&d);
        assert_eq!(b.color.as_deref(), Some("rgb(200, 200, 255)"));
        assert_eq!(b.label, "Team");
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
