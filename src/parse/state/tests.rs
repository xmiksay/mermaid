use super::*;
use crate::parse::State;

#[test]
fn simple_transitions() {
    let d = parse("stateDiagram-v2\n[*] --> Idle\nIdle --> Run: start\nRun --> [*]\n").unwrap();
    assert_eq!(d.states.len(), 4);
    assert_eq!(d.transitions.len(), 3);
}

#[test]
fn quoted_description_alias() {
    let d = parse("stateDiagram-v2\nstate \"This is a long name\" as s2\n[*] --> s2\ns2 --> [*]\n")
        .unwrap();
    // s2 + two `[*]` pseudo-states; no phantom `"…" as s2` box.
    assert_eq!(d.states.len(), 3);
    let s2 = d.states.iter().find(|s| s.id == "s2").unwrap();
    assert_eq!(s2.label, "This is a long name");
    assert!(!d.states.iter().any(|s| s.id.contains("as s2")));
}

#[test]
fn composite_state_block() {
    let d =
        parse("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> Sub\nSub --> [*]\n}\nA --> [*]\n")
            .unwrap();
    assert_eq!(d.composites.len(), 1);
    let c = &d.composites[0];
    assert_eq!(c.id, "A");
    assert_eq!(c.regions.len(), 1);
    assert!(c.regions[0].contains(&"Sub".to_string()));
}

#[test]
fn parallel_regions() {
    let d = parse(
        "stateDiagram-v2\nstate Combo {\n[*] --> X\nX --> [*]\n--\n[*] --> Y\nY --> [*]\n}\n",
    )
    .unwrap();
    assert_eq!(d.composites[0].regions.len(), 2);
}

#[test]
fn note_right_of() {
    let d = parse("stateDiagram-v2\nA --> B\nnote right of A: hello\n").unwrap();
    assert_eq!(d.notes.len(), 1);
    assert_eq!(d.notes[0].position, NotePosition::RightOf);
    assert_eq!(d.notes[0].target, "A");
    assert_eq!(d.notes[0].text, "hello");
}

#[test]
fn note_multiline() {
    let d = parse("stateDiagram-v2\nA --> B\nnote left of A\nthis is\na long note\nend note\n")
        .unwrap();
    assert_eq!(d.notes.len(), 1);
    assert!(d.notes[0].text.contains("this is"));
    assert!(d.notes[0].text.contains("long note"));
}

#[test]
fn stereotypes_recognized() {
    let d = parse(
        "stateDiagram\nstate fork_1 <<fork>>\nstate join_1 <<join>>\nstate c <<choice>>\nfork_1 --> join_1\n",
    )
    .unwrap();
    let kinds: Vec<_> = d.states.iter().map(|s| (s.id.clone(), s.kind)).collect();
    assert!(kinds.contains(&("fork_1".into(), StateKind::Fork)));
}

#[test]
fn history_stereotype() {
    let d = parse("stateDiagram-v2\nstate h <<history>>\n").unwrap();
    assert_eq!(state(&d, "h").kind, StateKind::History { deep: false });
}

#[test]
fn history_bracket_forms() {
    let d = parse("stateDiagram-v2\nstate A {\n[*] --> B\nB --> [H]\n[H*] --> C\nC --> [*]\n}\n")
        .unwrap();
    let kinds: Vec<_> = d.states.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&StateKind::History { deep: false }));
    assert!(kinds.contains(&StateKind::History { deep: true }));
}

#[test]
fn direction_parsed() {
    let d = parse("stateDiagram-v2\ndirection LR\nA --> B\n").unwrap();
    assert_eq!(d.direction, FlowDirection::LeftRight);
}

fn state<'a>(d: &'a StateDiagram, id: &str) -> &'a State {
    d.states.iter().find(|s| s.id == id).unwrap()
}

#[test]
fn classdef_class_and_style() {
    let d = parse(
        "stateDiagram-v2\n[*] --> A\nclassDef foo fill:#0f0\nclass A foo\nstyle A stroke:#333\n",
    )
    .unwrap();
    assert!(d.class_defs.contains_key("foo"));
    assert_eq!(state(&d, "A").classes, vec!["foo".to_string()]);
    assert_eq!(
        state(&d, "A").style,
        vec![("stroke".to_string(), "#333".to_string())]
    );
}

#[test]
fn bare_id_declaration() {
    let d = parse("stateDiagram-v2\ns1\ns2\ns1 --> s2\n").unwrap();
    // s1 + s2, no phantom states.
    assert_eq!(d.states.len(), 2);
    assert!(d.states.iter().any(|s| s.id == "s1"));
    assert!(d.states.iter().any(|s| s.id == "s2"));
    assert_eq!(d.transitions.len(), 1);
}

#[test]
fn bare_id_in_composite_region() {
    let d = parse("stateDiagram-v2\nstate Fork {\nc1\nc2\n}\n").unwrap();
    let regions = &d.composites[0].regions;
    assert!(regions[0].contains(&"c1".to_string()));
    assert!(regions[0].contains(&"c2".to_string()));
}

#[test]
fn multiword_statement_still_errors() {
    assert!(parse("stateDiagram-v2\nthis is garbage\n").is_err());
}

#[test]
fn bracket_fork_join_choice() {
    let d =
        parse("stateDiagram-v2\nstate f [[fork]]\nstate j [[join]]\nstate c [[choice]]\nf --> j\n")
            .unwrap();
    assert_eq!(state(&d, "f").kind, StateKind::Fork);
    assert_eq!(state(&d, "j").kind, StateKind::Join);
    assert_eq!(state(&d, "c").kind, StateKind::Choice);
    // No garbage `f [[fork]]` state.
    assert!(!d.states.iter().any(|s| s.id.contains('[')));
    assert_eq!(d.states.len(), 3);
}

#[test]
fn click_href_on_state() {
    let d = parse("stateDiagram-v2\n[*] --> A\nclick A href \"https://example.com\" \"open\"\n")
        .unwrap();
    assert_eq!(
        state(&d, "A").click,
        Some(crate::parse::ClickAction::Href {
            url: "https://example.com".into(),
            tooltip: Some("open".into()),
            target: None,
        })
    );
    // No phantom `//example.com"` state.
    assert!(!d.states.iter().any(|s| s.id.contains("example.com")));
}

#[test]
fn composite_quoted_alias() {
    let d =
        parse("stateDiagram-v2\nstate \"Composite label\" as C {\n[*] --> Inner\n}\nC --> [*]\n")
            .unwrap();
    // Composite id is `C`, labelled with the quoted text — no raw-text box.
    assert_eq!(d.composites[0].id, "C");
    assert_eq!(state(&d, "C").label, "Composite label");
    assert!(!d.states.iter().any(|s| s.id.contains('"')));
    // Transition to C resolves to the composite, not a separate state.
    assert!(d.transitions.iter().any(|t| t.from == "C"));
}

#[test]
fn hide_empty_description_ignored() {
    let d = parse("stateDiagram-v2\nhide empty description\n[*] --> A\n").unwrap();
    assert!(d.states.iter().any(|s| s.id == "A"));
}

#[test]
fn inline_class_on_transition() {
    let d = parse("stateDiagram-v2\nA:::foo --> B:::bar : go\n").unwrap();
    assert_eq!(state(&d, "A").classes, vec!["foo".to_string()]);
    assert_eq!(state(&d, "B").classes, vec!["bar".to_string()]);
    assert_eq!(d.transitions[0].label.as_deref(), Some("go"));
}
