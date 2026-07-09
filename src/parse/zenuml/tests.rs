use super::super::ast::{ParticipantKind, SequenceDiagram, SequenceItem};
use super::super::ParseError;
use super::{parse, DEFAULT_STARTER};

fn parse_ok(src: &str) -> SequenceDiagram {
    parse(src).unwrap()
}

#[test]
fn annotator_declares_actor_and_starter() {
    let d = parse_ok("zenuml\n@Actor Alice\n@Database DB\n@Starter(Alice)\nDB.query()\n");
    let alice = d.participants.iter().find(|p| p.id == "Alice").unwrap();
    assert_eq!(alice.kind, ParticipantKind::Actor);
    // The call originates from the declared starter, not the synthetic one.
    assert!(matches!(
        d.items.first(),
        Some(SequenceItem::Message(m)) if m.from == "Alice" && m.to == "DB"
    ));
    assert!(d.participants.iter().all(|p| p.id != DEFAULT_STARTER));
}

#[test]
fn comments_are_stripped() {
    let d = parse_ok("zenuml\n// a comment\nA.b() // trailing\n");
    assert_eq!(
        d.items
            .iter()
            .filter(|i| matches!(i, SequenceItem::Message(_)))
            .count(),
        1
    );
}

#[test]
fn stereotype_annotators_set_participant_kind() {
    let d =
        parse_ok("zenuml\n@Boundary UI\n@Control Ctrl\n@Entity Order\n@Database DB\nUI.click()\n");
    let kind = |id: &str| d.participants.iter().find(|p| p.id == id).unwrap().kind;
    assert_eq!(kind("UI"), ParticipantKind::Boundary);
    assert_eq!(kind("Ctrl"), ParticipantKind::Control);
    assert_eq!(kind("Order"), ParticipantKind::Entity);
    assert_eq!(kind("DB"), ParticipantKind::Database);
}

#[test]
fn bare_and_alias_declarations() {
    let d = parse_ok("zenuml\nBob\nA as Alice\nA.greet()\n");
    // Declaration order is column order: Bob, then A.
    assert_eq!(d.participants[0].id, "Bob");
    let a = d.participants.iter().find(|p| p.id == "A").unwrap();
    assert_eq!(a.display, "Alice");
    // The declarations produced no phantom Starter self-message; only the
    // real `A.greet()` call remains (from the implicit starter to A).
    let msgs: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            SequenceItem::Message(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(msgs.len(), 1);
    assert_eq!((&*msgs[0].from, &*msgs[0].to), (DEFAULT_STARTER, "A"));
}

#[test]
fn new_materializes_participant_with_creation_message() {
    let d = parse_ok("zenuml\nnew A1\nnew A2(with, parameters)\n");
    assert!(d
        .items
        .iter()
        .any(|i| matches!(i, SequenceItem::Create(id) if id == "A1")));
    assert!(d
        .items
        .iter()
        .any(|i| matches!(i, SequenceItem::Create(id) if id == "A2")));
    // Each `new` draws a creation message to the new participant, not a
    // Starter self-call.
    let create_msg = d
        .items
        .iter()
        .filter_map(|i| match i {
            SequenceItem::Message(m) if m.to == "A1" => Some(m),
            _ => None,
        })
        .next()
        .unwrap();
    assert_ne!(&*create_msg.from, "A1");
    assert_eq!(create_msg.text, "«create»");
}

#[test]
fn foreach_is_a_loop_keyword() {
    let d = parse_ok("zenuml\nforeach (item) {\n  A.step()\n}\n");
    assert!(matches!(
        d.items.first(),
        // A.step() is a method call: message + activate/deactivate band.
        Some(SequenceItem::Loop(b)) if b.label == "item"
            && matches!(b.items.first(), Some(SequenceItem::Message(_)))
    ));
}

#[test]
fn rejects_missing_header() {
    assert!(matches!(
        parse("flowchart TD\n"),
        Err(ParseError::Syntax { line: 1, .. })
    ));
}
