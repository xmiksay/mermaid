//! Per-line statement helpers: note parsing (`note … of X`), `classDef`/`class`/
//! `style` styling directives, and `A --> B` transition parsing.

use std::collections::HashMap;

use crate::parse::style::{parse_multi_id_stmt, parse_style_props};
use crate::parse::{NotePosition, ParseError, StateDiagram, StateKind, StateNote, StateTransition};

use super::decl::{apply_state_class, canonicalize, ensure_state, extract_inline_class};

pub(super) fn try_note_oneline(line: &str) -> Option<StateNote> {
    let body = line.strip_prefix("note ")?;
    let (head, text) = body.split_once(':')?;
    let (pos, target) = parse_note_head(head.trim())?;
    Some(StateNote {
        target,
        position: pos,
        text: text.trim().to_string(),
    })
}

pub(super) fn try_note_multiline(line: &str) -> Option<(String, NotePosition)> {
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
pub(super) fn handle_class_def(rest: &str, diag: &mut StateDiagram) {
    let Some((names, props)) = parse_multi_id_stmt(rest, false) else {
        return;
    };
    let style = parse_style_props(props);
    for name in names {
        diag.class_defs.insert(name, style.clone());
    }
}

/// `class <id1>,<id2> <className>` — apply a class to states.
pub(super) fn handle_class_apply(
    rest: &str,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
) {
    let Some((ids, class_name)) = parse_multi_id_stmt(rest, true) else {
        return;
    };
    for id in ids {
        ensure_state(diag, existing, &id, "", StateKind::Normal);
        apply_state_class(diag, existing, &id, class_name);
    }
}

/// `style <id> <props>` — inline style on a single state.
pub(super) fn handle_style(
    rest: &str,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
) {
    let Some((id, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let id = id.trim();
    ensure_state(diag, existing, id, "", StateKind::Normal);
    if let Some(&i) = existing.get(id) {
        diag.states[i].style = parse_style_props(props);
    }
}

pub(super) fn parse_transition(
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
