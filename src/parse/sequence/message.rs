//! Per-line statement parsing: messages (with the `->>+`/`-->>-` activation
//! shorthand), participants, notes, autonumber, and actor menus.

use crate::parse::ast::{
    ArrowKind, AutoNumberConfig, Message, NotePosition, Participant, ParticipantKind,
    SequenceDiagram, SequenceItem, SequenceNote,
};
use crate::parse::token::parse_attr_pairs;
use crate::parse::ParseError;

const ARROWS: &[(&str, ArrowKind)] = &[
    ("<<-->>", ArrowKind::BiDashedArrow),
    ("<<->>", ArrowKind::BiSolidArrow),
    ("-->>", ArrowKind::DashedArrow),
    ("-->", ArrowKind::Dashed),
    ("--x", ArrowKind::DashedCross),
    ("--)", ArrowKind::DashedOpen),
    // v11.12.3+ half (single-barb) arrows, matching upstream
    // sequenceDiagram.jison spellings (#223). The barb is a *doubled* char —
    // `\\` (upper) or `//` (lower) — or a single barb behind a `|` shaft
    // (`|\`/`|/`). Dashed forms carry the extra dash on the shaft side, and the
    // eight reverse forms put the barb at the tail. Longest tokens first so the
    // dashed/pipe variants win over their solid/bare prefixes.
    ("--|\\", ArrowKind::DashedHalfArrowTop),
    ("--|/", ArrowKind::DashedHalfArrowBottom),
    ("--\\\\", ArrowKind::DashedHalfArrowTop),
    ("--//", ArrowKind::DashedHalfArrowBottom),
    ("\\|--", ArrowKind::DashedHalfArrowStartTop),
    ("/|--", ArrowKind::DashedHalfArrowStartBottom),
    ("\\\\--", ArrowKind::DashedHalfArrowStartTop),
    ("//--", ArrowKind::DashedHalfArrowStartBottom),
    ("->>", ArrowKind::SolidArrow),
    ("->", ArrowKind::Solid),
    ("-x", ArrowKind::Cross),
    ("-)", ArrowKind::Open),
    ("-|\\", ArrowKind::HalfArrowTop),
    ("-|/", ArrowKind::HalfArrowBottom),
    ("-\\\\", ArrowKind::HalfArrowTop),
    ("-//", ArrowKind::HalfArrowBottom),
    ("\\|-", ArrowKind::HalfArrowStartTop),
    ("/|-", ArrowKind::HalfArrowStartBottom),
    ("\\\\-", ArrowKind::HalfArrowStartTop),
    ("//-", ArrowKind::HalfArrowStartBottom),
];

pub(super) fn parse_line_to_items(
    line: &str,
    diag: &mut SequenceDiagram,
    line_no: usize,
) -> Result<Vec<SequenceItem>, ParseError> {
    // Both the space form (`title Demo`) and the legacy colon form
    // (`title: Demo`, upstream lexer `"title:"\s[^#\n;]+`).
    if let Some(rest) = line
        .strip_prefix("title ")
        .or_else(|| line.strip_prefix("title:"))
    {
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

    // Actor metadata (`link A: Label @ url`, `links A: {json}`,
    // `properties A: {json}`, `details A: {json}`) is consumed but not rendered
    // — accepting the syntax keeps it from being a hard error.
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
    // Activation shorthand, both *after* the message (upstream jison
    // `actor signaltype +/- actor text`): `->>+B` activates the receiver
    // (`msg.to`), `-->>-B` deactivates the *sender* (`msg.from`) — the
    // participant that was activated when it earlier received a message.
    let receiver = msg.to.clone();
    let sender = msg.from.clone();
    let mut items = Vec::new();
    match activation {
        Activation::Activate => {
            items.push(SequenceItem::Message(msg));
            items.push(SequenceItem::Activate(receiver));
        }
        Activation::Deactivate => {
            items.push(SequenceItem::Message(msg));
            items.push(SequenceItem::Deactivate(sender));
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
        start: nums.next().and_then(|s| s.parse().ok()).unwrap_or(1.0),
        step: nums.next().and_then(|s| s.parse().ok()).unwrap_or(1.0),
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

/// `link <id>: <label> @ <url>`, `links <id>: {json}`, `properties <id>: {json}`
/// and `details <id>: {json}` attach popup-menu / metadata to an actor. Recognize
/// the shape (one of those keywords, an id, then a colon) so the line is consumed
/// rather than mistaken for a message.
fn is_actor_menu(line: &str) -> bool {
    let rest = ["links ", "link ", "properties ", "details "]
        .iter()
        .find_map(|kw| line.strip_prefix(kw));
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
        return Err(ParseError::malformed(line_no, "missing participant id"));
    }
    // v11.12+ metadata: `id@{ "type": "database" }` sets the participant type
    // (drawn with the matching stereotype glyph) without leaking the raw block
    // into the id.
    let (decl, meta_kind) = split_participant_meta(s);
    let kind = meta_kind.unwrap_or(kind);
    let decl = decl.trim();
    if decl.is_empty() {
        return Err(ParseError::malformed(line_no, "missing participant id"));
    }
    if let Some((id, alias)) = decl.split_once(" as ") {
        Ok(Participant {
            id: id.trim().to_string(),
            display: alias.trim().to_string(),
            kind,
        })
    } else {
        Ok(Participant {
            id: decl.to_string(),
            display: decl.to_string(),
            kind,
        })
    }
}

/// Split a v11.12+ `@{ … }` metadata block off a participant declaration,
/// returning the declaration with the block removed and the [`ParticipantKind`]
/// its `type` implies (if any). Upstream `participant Db@{ "type": "database" }`.
fn split_participant_meta(s: &str) -> (String, Option<ParticipantKind>) {
    let Some(at) = s.find("@{") else {
        return (s.to_string(), None);
    };
    let Some(close_rel) = s[at + 2..].find('}') else {
        return (s.to_string(), None);
    };
    let close = at + 2 + close_rel;
    let kind = meta_type_kind(&s[at + 2..close]);
    let mut decl = String::from(&s[..at]);
    decl.push_str(&s[close + 1..]);
    (decl, kind)
}

/// Read the `type` value out of a `{ "type": "database" }` metadata body and map
/// it onto a [`ParticipantKind`]. Unknown/absent types return `None` so the
/// caller keeps the declared `participant`/`actor` kind.
fn meta_type_kind(meta: &str) -> Option<ParticipantKind> {
    let value = parse_attr_pairs(meta)
        .into_iter()
        .find(|(k, _)| k == "type")?
        .1;
    match value.to_ascii_lowercase().as_str() {
        "boundary" => Some(ParticipantKind::Boundary),
        "control" => Some(ParticipantKind::Control),
        "entity" => Some(ParticipantKind::Entity),
        "database" | "db" => Some(ParticipantKind::Database),
        "actor" => Some(ParticipantKind::Actor),
        "participant" => Some(ParticipantKind::Participant),
        _ => None,
    }
}

/// Activation shorthand attached to a message arrow (`->>+` / `-->>-`).
enum Activation {
    None,
    Activate,
    Deactivate,
}

fn parse_message(line: &str, line_no: usize) -> Result<(Message, Activation), ParseError> {
    let (arrow_pos, token, kind) = find_arrow(line).ok_or_else(|| {
        ParseError::unknown(line_no, format!("not a recognized statement: '{line}'"))
    })?;
    let from = line[..arrow_pos].trim().to_string();
    if from.is_empty() {
        return Err(ParseError::malformed(line_no, "empty sender"));
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
        return Err(ParseError::malformed(line_no, "empty receiver"));
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
    fn half_arrows_recognized() {
        // v11.12.3+ single-barb half arrows spelled as upstream's jison does
        // (#223): the barb is a *doubled* char (`\\`/`//`) or a single barb
        // behind a `|` shaft; dashed forms add a dash; reverse forms put the
        // barb at the tail. `\` is the upper barb, `/` the lower one.
        //
        // Note: `\\` in these Rust string literals is one backslash, so `A-\\\\B`
        // is the two-backslash source `A-\\B`.
        let cases = [
            // forward, solid
            ("A-\\\\B: t", ArrowKind::HalfArrowTop),
            ("A-//B: t", ArrowKind::HalfArrowBottom),
            ("A-|\\B: t", ArrowKind::HalfArrowTop),
            ("A-|/B: t", ArrowKind::HalfArrowBottom),
            // forward, dashed
            ("A--\\\\B: t", ArrowKind::DashedHalfArrowTop),
            ("A--//B: t", ArrowKind::DashedHalfArrowBottom),
            ("A--|\\B: t", ArrowKind::DashedHalfArrowTop),
            ("A--|/B: t", ArrowKind::DashedHalfArrowBottom),
            // reverse, solid
            ("A\\\\-B: t", ArrowKind::HalfArrowStartTop),
            ("A//-B: t", ArrowKind::HalfArrowStartBottom),
            ("A\\|-B: t", ArrowKind::HalfArrowStartTop),
            ("A/|-B: t", ArrowKind::HalfArrowStartBottom),
            // reverse, dashed
            ("A\\\\--B: t", ArrowKind::DashedHalfArrowStartTop),
            ("A//--B: t", ArrowKind::DashedHalfArrowStartBottom),
            ("A\\|--B: t", ArrowKind::DashedHalfArrowStartTop),
            ("A/|--B: t", ArrowKind::DashedHalfArrowStartBottom),
        ];
        for (msg, expected) in cases {
            let s = format!("sequenceDiagram\n{msg}\n");
            let d = parse(&s).unwrap();
            assert_eq!(first_msg(&d).arrow, expected, "case: {msg}");
            // Clean endpoints — the barb never leaks into a participant id.
            assert_eq!(first_msg(&d).from, "A", "case: {msg}");
            assert_eq!(first_msg(&d).to, "B", "case: {msg}");
        }
    }

    #[test]
    fn wrong_single_char_half_barbs_are_not_arrows() {
        // The pre-#223 single-char spellings (`-\`, `-/`) are NOT upstream
        // tokens: `A-\B` used to be parsed as a half arrow but must now be a
        // hard error (no arrow token, so not a recognized statement).
        for msg in ["A-\\B: x", "A-/B: x"] {
            let s = format!("sequenceDiagram\n{msg}\n");
            assert!(parse(&s).is_err(), "case: {msg}");
        }
    }

    #[test]
    fn half_arrow_no_phantom_participant() {
        // Repros from #223: the doubled barb must be fully consumed, not leak a
        // `\Bob`/`/Bob` phantom, and reverse forms must not hard-error.
        let d = parse("sequenceDiagram\nA-\\\\Bob: x\n").unwrap();
        assert_eq!(d.participants.len(), 2);
        assert!(d.participants.iter().all(|p| p.id == "A" || p.id == "Bob"));

        let d = parse("sequenceDiagram\nA-//Bob: x\n").unwrap();
        assert!(d.participants.iter().all(|p| p.id == "A" || p.id == "Bob"));

        let d = parse("sequenceDiagram\nBob\\\\-A: y\n").unwrap();
        let m = first_msg(&d);
        assert_eq!(m.from, "Bob");
        assert_eq!(m.to, "A");
        assert_eq!(m.arrow, ArrowKind::HalfArrowStartTop);
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

        // `+` activates the receiver *after* the message; `-` deactivates the
        // *sender* *after* the message. So B is activated then closed:
        // Message(A->B), Activate(B), Message(B->A), Deactivate(B).
        assert!(matches!(&d.items[0], SequenceItem::Message(m) if m.to == "B"));
        assert!(matches!(&d.items[1], SequenceItem::Activate(s) if s == "B"));
        assert!(matches!(&d.items[2], SequenceItem::Message(m) if m.to == "A"));
        assert!(matches!(&d.items[3], SequenceItem::Deactivate(s) if s == "B"));
        assert_eq!(d.items.len(), 4);
    }

    #[test]
    fn deactivation_shorthand_closes_sender_band() {
        // Canonical docs example: John is activated by the first message and the
        // trailing `-` on the reply must close *John's* band, not Alice's.
        let d = parse("sequenceDiagram\nAlice->>+John: hi\nJohn-->>-Alice: ok\n").unwrap();
        assert!(matches!(&d.items[1], SequenceItem::Activate(s) if s == "John"));
        assert!(matches!(&d.items[3], SequenceItem::Deactivate(s) if s == "John"));
    }

    #[test]
    fn title_colon_form() {
        let d = parse("sequenceDiagram\ntitle: Demo Title\nA->>B: hi\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("Demo Title"));
    }

    #[test]
    fn participant_type_metadata_sets_kind() {
        let d = parse("sequenceDiagram\nparticipant Db@{ \"type\": \"database\" }\nDb->>Db: q\n")
            .unwrap();
        // One clean `Db` participant, no phantom raw-metadata id.
        assert_eq!(d.participants.len(), 1);
        let p = &d.participants[0];
        assert_eq!(p.id, "Db");
        assert_eq!(p.kind, ParticipantKind::Database);
    }

    #[test]
    fn participant_metadata_with_alias() {
        let d =
            parse("sequenceDiagram\nparticipant Q@{ \"type\": \"boundary\" } as Queue\nQ->>Q: x\n")
                .unwrap();
        let p = &d.participants[0];
        assert_eq!(p.id, "Q");
        assert_eq!(p.display, "Queue");
        assert_eq!(p.kind, ParticipantKind::Boundary);
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
            Some(SequenceItem::AutoNumber(Some(c))) if c.start == 1.0 && c.step == 1.0
        ));
    }

    #[test]
    fn autonumber_start_and_step() {
        let d = parse("sequenceDiagram\nautonumber 10 5\nA->>B: x\n").unwrap();
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::AutoNumber(Some(c))) if c.start == 10.0 && c.step == 5.0
        ));
    }

    #[test]
    fn autonumber_decimal_start_and_step() {
        // v11.15+ accepts fractional start/step (#176) — previously these fell
        // back to 1/1.
        let d = parse("sequenceDiagram\nautonumber 1.5 0.5\nA->>B: x\n").unwrap();
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::AutoNumber(Some(c))) if c.start == 1.5 && c.step == 0.5
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

    #[test]
    fn properties_and_details_lines_are_consumed() {
        // `properties A: {..}` / `details A: {..}` attach actor metadata and must
        // not hard-error (#176).
        let d = parse(
            "sequenceDiagram\nparticipant A\nproperties A: {\"class\": \"internal\"}\ndetails A: {\"key\": \"value\"}\nA->>A: x\n",
        )
        .unwrap();
        assert_eq!(
            d.items
                .iter()
                .filter(|i| matches!(i, SequenceItem::Message(_)))
                .count(),
            1
        );
    }
}
