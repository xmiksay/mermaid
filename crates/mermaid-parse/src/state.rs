//! State diagram parser (v0.1 subset).
//!
//! Supports:
//!   * `stateDiagram` and `stateDiagram-v2` headers.
//!   * Transitions `A --> B` and `A --> B : label`.
//!   * `[*]` start and end markers — each occurrence becomes its own
//!     pseudo-state node (so a diagram can have multiple start/end indicators).
//!   * State declarations `state X` and `state X : description`.
//!   * Stereotypes via `state X <<choice>>` (choice/fork/join).
//!   * `direction TB|TD|BT|LR|RL`.
//!
//! Composite state blocks (`state X { ... }`) are flattened: opener/closer
//! lines are skipped, nested content is ignored. Properly nesting would
//! require sub-graph support in the renderer.

use std::collections::HashMap;

use crate::ast::{FlowDirection, State, StateDiagram, StateKind, StateTransition};
use crate::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<StateDiagram, ParseError> {
    let mut diag = StateDiagram::default();
    let mut header_seen = false;
    let mut depth: usize = 0;
    let mut start_n = 0usize;
    let mut end_n = 0usize;
    let mut existing: HashMap<String, usize> = HashMap::new();

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

        // Composite state block start/end — flatten by skipping.
        if line.ends_with('{') {
            depth += 1;
            continue;
        }
        if line == "}" {
            depth = depth.saturating_sub(1);
            continue;
        }
        if depth > 0 {
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
            parse_transition(line, &mut diag, &mut existing, &mut start_n, &mut end_n)?;
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

fn parse_state_decl(
    rest: &str,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
) {
    // Cases:
    //   X
    //   X : description
    //   X <<choice>>
    //   X <<fork>>
    //   X <<join>>
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
) -> Result<(), ParseError> {
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
        from: from_id,
        to: to_id,
        label,
    });
    Ok(())
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
    let final_label = if label.is_empty() { id.to_string() } else { label.to_string() };
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
        let s = "stateDiagram-v2\n[*] --> Idle\nIdle --> Running: start\nRunning --> [*]\n";
        let d = parse(s).unwrap();
        // 2 [*] occurrences → 2 pseudo states; Idle, Running → 4 total
        assert_eq!(d.states.len(), 4);
        assert_eq!(d.transitions.len(), 3);
        assert_eq!(d.transitions[1].label.as_deref(), Some("start"));
    }

    #[test]
    fn stereotypes_recognized() {
        let s = "stateDiagram\nstate fork_1 <<fork>>\nstate join_1 <<join>>\nstate c <<choice>>\nfork_1 --> join_1\n";
        let d = parse(s).unwrap();
        let kinds: Vec<_> = d.states.iter().map(|s| (s.id.clone(), s.kind)).collect();
        assert!(kinds.contains(&("fork_1".into(), StateKind::Fork)));
        assert!(kinds.contains(&("join_1".into(), StateKind::Join)));
        assert!(kinds.contains(&("c".into(), StateKind::Choice)));
    }

    #[test]
    fn direction_parsed() {
        let s = "stateDiagram-v2\ndirection LR\nA --> B\n";
        let d = parse(s).unwrap();
        assert_eq!(d.direction, FlowDirection::LeftRight);
    }

    #[test]
    fn composite_block_flattened() {
        let s = "stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> Sub\nSub --> [*]\n}\nA --> [*]\n";
        let d = parse(s).unwrap();
        // The nested [*]/Sub statements are skipped; only A and [*] markers visible.
        let names: Vec<_> = d.states.iter().map(|s| s.id.clone()).collect();
        assert!(names.contains(&"A".to_string()));
        assert!(!names.contains(&"Sub".to_string()));
    }

    #[test]
    fn description_on_state() {
        let s = "stateDiagram-v2\nstate X : doing work\nX --> Y\n";
        let d = parse(s).unwrap();
        let x = d.states.iter().find(|s| s.id == "X").unwrap();
        assert_eq!(x.label, "doing work");
    }
}
