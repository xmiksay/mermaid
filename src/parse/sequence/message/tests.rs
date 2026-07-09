use super::super::parse;
use crate::parse::ast::{
    ArrowKind, Message, NotePosition, ParticipantKind, SequenceDiagram, SequenceItem,
};

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
    let d =
        parse("sequenceDiagram\nA->>B: hi\nNote over A,B: shared\nNote right of B: thx\n").unwrap();
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
    let d =
        parse("sequenceDiagram\nparticipant Db@{ \"type\": \"database\" }\nDb->>Db: q\n").unwrap();
    // One clean `Db` participant, no phantom raw-metadata id.
    assert_eq!(d.participants.len(), 1);
    let p = &d.participants[0];
    assert_eq!(p.id, "Db");
    assert_eq!(p.kind, ParticipantKind::Database);
}

#[test]
fn participant_metadata_with_alias() {
    let d = parse("sequenceDiagram\nparticipant Q@{ \"type\": \"boundary\" } as Queue\nQ->>Q: x\n")
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
