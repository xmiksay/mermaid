//! Sequence diagram parser.
//!
//! Supports:
//!   * `sequenceDiagram` header.
//!   * `title <text>`.
//!   * `participant <id> [as <alias>]`, `actor <id> [as <alias>]`.
//!   * Messages: `from <arrow> to : text` with arrows
//!     `->`, `->>`, `-->`, `-->>`, `-x`, `--x`, `-)`, `--)`, the bidirectional
//!     forms `<<->>`, `<<-->>`, and the v11.12.3+ half (single-barb) arrows
//!     `-\`, `-/`, `-|\`, `-|/` (dashed variants with the extra leading dash).
//!   * `autonumber [start [step]]` — `start`/`step` may be fractional
//!     (`autonumber 1.5 0.5`, v11.15+).
//!   * `activate <id>` / `deactivate <id>`, plus the `->>+`/`-->>-` arrow
//!     activation shorthand.
//!   * `create [participant|actor] <id> [as <alias>]` / `destroy <id>` —
//!     participant lifecycle (positional items, spawned/terminated inline).
//!   * `link <id>: …` / `links <id>: {…}` actor menus and `properties <id>: {…}`
//!     / `details <id>: {…}` actor metadata (consumed, not rendered).
//!   * Notes: `Note over A[,B]: text`, `Note right of A: text`, `Note left of A: text`.
//!   * Blocks (each followed by item lines, terminated by `end`):
//!     `alt label` ... `else label` ... `end`,
//!     `loop label` ... `end`,
//!     `par label` ... `and label` ... `end` (and the overlapping `par_over`),
//!     `opt label` ... `end`,
//!     `critical label` ... `option label` ... `end`.
//!   * `box label` ... `end` — collects participants declared inside.

mod frames;
mod message;

use frames::*;
use message::*;

use super::ast::{SequenceDiagram, SequenceItem};
use super::{strip_comment, ParseError};

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
                return Err(ParseError::header(
                    line_no,
                    "expected 'sequenceDiagram' header",
                ));
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
    reorder_destroys(&mut diag.items);
    Ok(diag)
}

/// `destroy X` marks a participant for destruction, but upstream keeps its
/// lifeline alive through the message that immediately follows and references
/// it (the classic `destroy Carl` / `Alice-xCarl` pattern). Move each
/// `Destroy` past the next message involving its target so the terminating
/// cross lands after that message rather than before it.
fn reorder_destroys(items: &mut Vec<SequenceItem>) {
    for it in items.iter_mut() {
        match it {
            SequenceItem::Alt(bs) | SequenceItem::Par(bs) | SequenceItem::Critical(bs) => {
                for b in bs {
                    reorder_destroys(&mut b.items);
                }
            }
            SequenceItem::Loop(b) | SequenceItem::Opt(b) | SequenceItem::Break(b) => {
                reorder_destroys(&mut b.items);
            }
            SequenceItem::Rect(r) => reorder_destroys(&mut r.items),
            _ => {}
        }
    }
    let mut i = 0;
    while i < items.len() {
        let id = match &items[i] {
            SequenceItem::Destroy(id) => id.clone(),
            _ => {
                i += 1;
                continue;
            }
        };
        let off = items[i + 1..]
            .iter()
            .position(|it| matches!(it, SequenceItem::Message(m) if m.from == id || m.to == id));
        match off {
            Some(off) => {
                let d = items.remove(i);
                // After removal the matching message sits at `i + off`; place
                // the Destroy right after it and skip past both.
                items.insert(i + off + 1, d);
                i += off + 2;
            }
            None => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn par_over_block() {
        // `par_over` is upstream's overlapping-par frame; it reuses the Par
        // branch structure and must not hard-error (#176).
        let d = parse("sequenceDiagram\npar_over shared\nA->>B: x\nand other\nA->>C: y\nend\n")
            .unwrap();
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
        assert_eq!(branches[0].label, "shared");
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
    fn destroy_moves_past_following_message() {
        // `destroy Carl` then `Alice-xCarl` — the Destroy must land *after* the
        // terminating message, matching upstream.
        let d = parse(
            "sequenceDiagram\ncreate participant Carl\nAlice->>Carl: Hi\ndestroy Carl\nAlice-xCarl: bye\n",
        )
        .unwrap();
        let destroy_pos = d
            .items
            .iter()
            .position(|i| matches!(i, SequenceItem::Destroy(id) if id == "Carl"))
            .unwrap();
        let last_msg_pos = d
            .items
            .iter()
            .rposition(|i| matches!(i, SequenceItem::Message(m) if m.text == "bye"))
            .unwrap();
        assert!(
            destroy_pos > last_msg_pos,
            "destroy should follow its message"
        );
    }

    #[test]
    fn destroy_without_following_message_stays_put() {
        let d = parse("sequenceDiagram\nA->>B: hi\ndestroy B\n").unwrap();
        assert!(matches!(d.items.last(), Some(SequenceItem::Destroy(id)) if id == "B"));
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
