//! State diagram parser.
//!
//! Supports:
//!   * `stateDiagram` and `stateDiagram-v2` headers.
//!   * `[*]` start/end pseudo-states (each occurrence gets a unique id).
//!   * Transitions `A --> B[: label]`.
//!   * `state X` and `state X : description` declarations.
//!   * Stereotypes via `state X <<choice/fork/join>>`.
//!   * `direction TB|TD|BT|LR|RL`.
//!   * Composite states `state X { ... }` (potentially nested), with
//!     parallel regions separated by `--`.
//!   * Notes: `note right of X: text`, `note left of X: text`,
//!     `note over X: text` (consumed across following lines until `end note`).

use std::collections::HashMap;

use super::ast::{
    CompositeState, FlowDirection, NotePosition, State, StateDiagram, StateKind, StateNote,
    StateTransition,
};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<StateDiagram, ParseError> {
    let mut diag = StateDiagram::default();
    let mut header_seen = false;
    let mut composite_stack: Vec<CompositeFrame> = Vec::new();
    let mut start_n = 0usize;
    let mut end_n = 0usize;
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
                return Err(ParseError::Syntax {
                    message: "expected 'stateDiagram' or 'stateDiagram-v2' header".into(),
                    line: line_no,
                });
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
            let id_part = inner
                .split_once("<<")
                .map(|(a, _)| a.trim())
                .unwrap_or(inner);
            ensure_state(&mut diag, &mut existing, id_part, "", StateKind::Normal);
            composite_stack.push(CompositeFrame {
                id: id_part.to_string(),
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
                    return Err(ParseError::Syntax {
                        message: format!("unknown direction: '{other}'"),
                        line: line_no,
                    })
                }
            };
            continue;
        }

        if let Some(rest) = line.strip_prefix("state ") {
            parse_state_decl(rest, &mut diag, &mut existing);
            continue;
        }

        if line.contains("-->") {
            let (from_id, to_id) =
                parse_transition(line, &mut diag, &mut existing, &mut start_n, &mut end_n)?;
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

        return Err(ParseError::Syntax {
            message: format!("unrecognized state statement: '{line}'"),
            line: line_no,
        });
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

fn parse_state_decl(rest: &str, diag: &mut StateDiagram, existing: &mut HashMap<String, usize>) {
    let rest = rest.trim();
    let (id_part, label_part) = match rest.split_once(':') {
        Some((a, b)) => (a.trim(), b.trim().to_string()),
        None => (rest, String::new()),
    };
    let (id, kind) = if let Some(idx) = id_part.find("<<") {
        let id = id_part[..idx].trim().to_string();
        let stereo = id_part[idx + 2..].trim_end_matches(">>").trim();
        let k = match stereo {
            "choice" => StateKind::Choice,
            "fork" => StateKind::Fork,
            "join" => StateKind::Join,
            _ => StateKind::Normal,
        };
        (id, k)
    } else {
        (id_part.to_string(), StateKind::Normal)
    };
    ensure_state(diag, existing, &id, &label_part, kind);
}

fn parse_transition(
    line: &str,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
    start_n: &mut usize,
    end_n: &mut usize,
) -> Result<(String, String), ParseError> {
    let arrow = "-->";
    let pos = line.find(arrow).unwrap();
    let from_raw = line[..pos].trim();
    let after = line[pos + arrow.len()..].trim();
    let (to_raw, label) = match after.split_once(':') {
        Some((a, b)) => (a.trim(), Some(b.trim().to_string())),
        None => (after, None),
    };
    let from_id = canonicalize(from_raw, true, diag, existing, start_n, end_n);
    let to_id = canonicalize(to_raw, false, diag, existing, start_n, end_n);
    diag.transitions.push(StateTransition {
        from: from_id.clone(),
        to: to_id.clone(),
        label,
    });
    Ok((from_id, to_id))
}

fn canonicalize(
    raw: &str,
    is_source: bool,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
    start_n: &mut usize,
    end_n: &mut usize,
) -> String {
    if raw == "[*]" {
        if is_source {
            *start_n += 1;
            let id = format!("__start_{start_n}");
            diag.states.push(State {
                id: id.clone(),
                label: String::new(),
                kind: StateKind::Start,
            });
            id
        } else {
            *end_n += 1;
            let id = format!("__end_{end_n}");
            diag.states.push(State {
                id: id.clone(),
                label: String::new(),
                kind: StateKind::End,
            });
            id
        }
    } else {
        ensure_state(diag, existing, raw, "", StateKind::Normal);
        raw.to_string()
    }
}

fn ensure_state(
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
    id: &str,
    label: &str,
    kind: StateKind,
) {
    if let Some(&i) = existing.get(id) {
        if !label.is_empty() && diag.states[i].label.is_empty() {
            diag.states[i].label = label.to_string();
        }
        if matches!(diag.states[i].kind, StateKind::Normal) && kind != StateKind::Normal {
            diag.states[i].kind = kind;
        }
        return;
    }
    existing.insert(id.to_string(), diag.states.len());
    let final_label = if label.is_empty() {
        id.to_string()
    } else {
        label.to_string()
    };
    diag.states.push(State {
        id: id.to_string(),
        label: final_label,
        kind,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_transitions() {
        let d = parse("stateDiagram-v2\n[*] --> Idle\nIdle --> Run: start\nRun --> [*]\n").unwrap();
        assert_eq!(d.states.len(), 4);
        assert_eq!(d.transitions.len(), 3);
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
    fn direction_parsed() {
        let d = parse("stateDiagram-v2\ndirection LR\nA --> B\n").unwrap();
        assert_eq!(d.direction, FlowDirection::LeftRight);
    }
}
