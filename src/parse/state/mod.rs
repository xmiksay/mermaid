//! State diagram parser.
//!
//! Supports:
//!   * `stateDiagram` and `stateDiagram-v2` headers.
//!   * `[*]` start/end pseudo-states (each occurrence gets a unique id).
//!   * `[H]`/`[H*]` history pseudo-states (shallow/deep, unique id each).
//!   * Transitions `A --> B[: label]`.
//!   * `state X`, `state X : description`, `state "description" as X`, and the
//!     bare id form `X` on its own line.
//!   * Stereotypes via `state X <<choice/fork/join/history>>` and the
//!     `[[fork]]`/`[[join]]`/`[[choice]]` bracket alternates.
//!   * `direction TB|TD|BT|LR|RL`.
//!   * Composite states `state X { ... }` (potentially nested), with
//!     parallel regions separated by `--`; the composite header accepts the
//!     `state "label" as X {` aliasing form.
//!   * `click X href "url"` / `click X call fn()` interactions (reuses the
//!     flowchart `ClickAction`), and the `hide empty description` no-op.
//!   * Notes: `note right of X: text`, `note left of X: text`,
//!     `note over X: text` (consumed across following lines until `end note`).

use std::collections::HashMap;

use super::ast::{
    CompositeState, FlowDirection, NotePosition, StateDiagram, StateKind, StateNote,
    StateTransition,
};
use super::flowchart::click::parse_click;
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

mod decl;
use decl::*;

pub(crate) fn parse(input: &str) -> Result<StateDiagram, ParseError> {
    let mut diag = StateDiagram::default();
    let mut header_seen = false;
    let mut composite_stack: Vec<CompositeFrame> = Vec::new();
    let mut start_n = 0usize;
    let mut end_n = 0usize;
    let mut hist_n = 0usize;
    let mut existing: HashMap<String, usize> = HashMap::new();
    let mut pending_note: Option<(String, NotePosition, Vec<String>)> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if !(line == "stateDiagram" || line == "stateDiagram-v2") {
                return Err(ParseError::header(
                    line_no,
                    "expected 'stateDiagram' or 'stateDiagram-v2' header",
                ));
            }
            header_seen = true;
            continue;
        }

        if let Some((target, pos, text_lines)) = pending_note.as_mut() {
            if line == "end note" {
                let note = StateNote {
                    target: std::mem::take(target),
                    position: *pos,
                    text: text_lines.join(" "),
                };
                diag.notes.push(note);
                pending_note = None;
                continue;
            }
            text_lines.push(line.to_string());
            continue;
        }

        if let Some(note) = try_note_oneline(line) {
            diag.notes.push(note);
            continue;
        }
        if let Some((target, pos)) = try_note_multiline(line) {
            pending_note = Some((target, pos, Vec::new()));
            continue;
        }

        if line == "}" {
            if let Some(frame) = composite_stack.pop() {
                // Push composite into diag
                let composite = CompositeState {
                    id: frame.id.clone(),
                    regions: frame.regions,
                };
                diag.composites.push(composite);
                // Ensure parent composite, if any, records child id
                if let Some(parent) = composite_stack.last_mut() {
                    if let Some(region) = parent.regions.last_mut() {
                        if !region.contains(&frame.id) {
                            region.push(frame.id.clone());
                        }
                    }
                }
            }
            continue;
        }

        if line == "--" {
            // Open a new region inside the topmost composite.
            if let Some(top) = composite_stack.last_mut() {
                top.regions.push(Vec::new());
            }
            continue;
        }

        if line.ends_with('{') && line.starts_with("state ") {
            let inner = line
                .strip_prefix("state ")
                .unwrap()
                .trim_end_matches('{')
                .trim();
            // `state "Composite label" as C {` aliases the composite id `C` to a
            // display label; otherwise the id is the text before any `<<…>>`.
            let (id_part, label) = match parse_quoted_as(inner) {
                Some((id, desc)) => (id, desc),
                None => (
                    inner
                        .split_once("<<")
                        .map(|(a, _)| a.trim())
                        .unwrap_or(inner)
                        .to_string(),
                    String::new(),
                ),
            };
            ensure_state(
                &mut diag,
                &mut existing,
                &id_part,
                &label,
                StateKind::Normal,
            );
            composite_stack.push(CompositeFrame {
                id: id_part,
                regions: vec![Vec::new()],
            });
            continue;
        }

        if let Some(d) = line.strip_prefix("direction ") {
            diag.direction = match d.trim() {
                "TB" | "TD" => FlowDirection::TopDown,
                "BT" => FlowDirection::BottomTop,
                "LR" => FlowDirection::LeftRight,
                "RL" => FlowDirection::RightLeft,
                other => {
                    return Err(ParseError::unknown(
                        line_no,
                        format!("unknown direction: '{other}'"),
                    ))
                }
            };
            continue;
        }

        if let Some(rest) = line.strip_prefix("classDef ") {
            handle_class_def(rest, &mut diag);
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            handle_class_apply(rest, &mut diag, &mut existing);
            continue;
        }
        if let Some(rest) = line.strip_prefix("style ") {
            handle_style(rest, &mut diag, &mut existing);
            continue;
        }

        // `hide empty description` toggles rendering of empty state descriptions
        // upstream; the static renderer always draws the id, so it is a no-op.
        if line == "hide empty description" {
            continue;
        }

        if let Some(rest) = line.strip_prefix("click ") {
            let Some((id, action)) = parse_click(rest) else {
                return Err(ParseError::malformed(
                    line_no,
                    format!("malformed click directive: '{line}'"),
                ));
            };
            ensure_state(&mut diag, &mut existing, &id, "", StateKind::Normal);
            if let Some(&i) = existing.get(&id) {
                diag.states[i].click = Some(action);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("state ") {
            parse_state_decl(rest, &mut diag, &mut existing);
            continue;
        }

        if line.contains("-->") {
            let (from_id, to_id) = parse_transition(
                line,
                &mut diag,
                &mut existing,
                &mut start_n,
                &mut end_n,
                &mut hist_n,
            )?;
            // Composite tracking: each new normal state declared in the line
            // belongs to the active region.
            if let Some(top) = composite_stack.last_mut() {
                if let Some(region) = top.regions.last_mut() {
                    for id in [&from_id, &to_id] {
                        if existing.contains_key(id) && !region.contains(id) {
                            region.push(id.clone());
                        }
                    }
                }
            }
            continue;
        }

        // Stand-alone description: "X : text"
        if let Some((id, desc)) = line.split_once(':') {
            let id = id.trim().to_string();
            let label = desc.trim().to_string();
            ensure_state(&mut diag, &mut existing, &id, &label, StateKind::Normal);
            continue;
        }

        // Bare state-id declaration: `s1` on its own line (upstream's
        // `statement: idStatement`), common inside concurrency examples.
        if is_bare_id(line) {
            ensure_state(&mut diag, &mut existing, line, "", StateKind::Normal);
            if let Some(top) = composite_stack.last_mut() {
                if let Some(region) = top.regions.last_mut() {
                    if !region.contains(&line.to_string()) {
                        region.push(line.to_string());
                    }
                }
            }
            continue;
        }

        return Err(ParseError::unknown(
            line_no,
            format!("unrecognized state statement: '{line}'"),
        ));
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

struct CompositeFrame {
    id: String,
    regions: Vec<Vec<String>>,
}

/// A bare state-id declaration is a single identifier token — no whitespace and
/// only identifier characters — so a genuinely unknown multi-word statement
/// still hard-errors rather than silently becoming a phantom state.
fn is_bare_id(line: &str) -> bool {
    !line.is_empty()
        && line
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn try_note_oneline(line: &str) -> Option<StateNote> {
    let body = line.strip_prefix("note ")?;
    let (head, text) = body.split_once(':')?;
    let (pos, target) = parse_note_head(head.trim())?;
    Some(StateNote {
        target,
        position: pos,
        text: text.trim().to_string(),
    })
}

fn try_note_multiline(line: &str) -> Option<(String, NotePosition)> {
    // `note right of X` without colon means following lines until `end note`.
    let body = line.strip_prefix("note ")?;
    if body.contains(':') {
        return None;
    }
    let (pos, target) = parse_note_head(body.trim())?;
    Some((target, pos))
}

fn parse_note_head(head: &str) -> Option<(NotePosition, String)> {
    if let Some(t) = head.strip_prefix("right of ") {
        return Some((NotePosition::RightOf, t.trim().to_string()));
    }
    if let Some(t) = head.strip_prefix("left of ") {
        return Some((NotePosition::LeftOf, t.trim().to_string()));
    }
    if let Some(t) = head.strip_prefix("over ") {
        return Some((NotePosition::Over, t.trim().to_string()));
    }
    None
}

/// `classDef <name>[,<name2>] <props>` — define style classes.
fn handle_class_def(rest: &str, diag: &mut StateDiagram) {
    let Some((names, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let style = parse_style_props(props);
    for name in names.split(',') {
        let name = name.trim();
        if !name.is_empty() {
            diag.class_defs.insert(name.to_string(), style.clone());
        }
    }
}

/// `class <id1>,<id2> <className>` — apply a class to states.
fn handle_class_apply(rest: &str, diag: &mut StateDiagram, existing: &mut HashMap<String, usize>) {
    let Some((ids, class_name)) = rest.trim().rsplit_once(char::is_whitespace) else {
        return;
    };
    let class_name = class_name.trim();
    if class_name.is_empty() {
        return;
    }
    for id in ids.split(',') {
        let id = id.trim();
        if !id.is_empty() {
            ensure_state(diag, existing, id, "", StateKind::Normal);
            apply_state_class(diag, existing, id, class_name);
        }
    }
}

/// `style <id> <props>` — inline style on a single state.
fn handle_style(rest: &str, diag: &mut StateDiagram, existing: &mut HashMap<String, usize>) {
    let Some((id, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let id = id.trim();
    ensure_state(diag, existing, id, "", StateKind::Normal);
    if let Some(&i) = existing.get(id) {
        diag.states[i].style = parse_style_props(props);
    }
}

fn parse_transition(
    line: &str,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
    start_n: &mut usize,
    end_n: &mut usize,
    hist_n: &mut usize,
) -> Result<(String, String), ParseError> {
    let arrow = "-->";
    let pos = line.find(arrow).unwrap();
    let (from_raw, from_class) = extract_inline_class(line[..pos].trim());
    let (after, to_class) = extract_inline_class(line[pos + arrow.len()..].trim());
    let (to_raw, label) = match after.split_once(':') {
        Some((a, b)) => (a.trim(), Some(b.trim().to_string())),
        None => (after.as_str(), None),
    };
    let from_id = canonicalize(&from_raw, true, diag, existing, start_n, end_n, hist_n);
    let to_id = canonicalize(to_raw, false, diag, existing, start_n, end_n, hist_n);
    if let Some(cls) = from_class {
        apply_state_class(diag, existing, &from_id, &cls);
    }
    if let Some(cls) = to_class {
        apply_state_class(diag, existing, &to_id, &cls);
    }
    diag.transitions.push(StateTransition {
        from: from_id.clone(),
        to: to_id.clone(),
        label,
    });
    Ok((from_id, to_id))
}

#[cfg(test)]
mod tests {
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
        let d =
            parse("stateDiagram-v2\nstate \"This is a long name\" as s2\n[*] --> s2\ns2 --> [*]\n")
                .unwrap();
        // s2 + two `[*]` pseudo-states; no phantom `"…" as s2` box.
        assert_eq!(d.states.len(), 3);
        let s2 = d.states.iter().find(|s| s.id == "s2").unwrap();
        assert_eq!(s2.label, "This is a long name");
        assert!(!d.states.iter().any(|s| s.id.contains("as s2")));
    }

    #[test]
    fn composite_state_block() {
        let d = parse(
            "stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> Sub\nSub --> [*]\n}\nA --> [*]\n",
        )
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
        let d =
            parse("stateDiagram-v2\nstate A {\n[*] --> B\nB --> [H]\n[H*] --> C\nC --> [*]\n}\n")
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
        let d = parse(
            "stateDiagram-v2\nstate f [[fork]]\nstate j [[join]]\nstate c [[choice]]\nf --> j\n",
        )
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
        let d =
            parse("stateDiagram-v2\n[*] --> A\nclick A href \"https://example.com\" \"open\"\n")
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
        let d = parse(
            "stateDiagram-v2\nstate \"Composite label\" as C {\n[*] --> Inner\n}\nC --> [*]\n",
        )
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
}
