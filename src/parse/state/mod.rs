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

use super::ast::{CompositeState, FlowDirection, NotePosition, StateDiagram, StateKind, StateNote};
use super::flowchart::click::parse_click;
use super::{strip_comment, ParseError};

mod decl;
use decl::*;

mod stmt;
use stmt::{
    handle_class_apply, handle_class_def, handle_style, parse_transition, try_note_multiline,
    try_note_oneline,
};

#[cfg(test)]
mod tests;

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
