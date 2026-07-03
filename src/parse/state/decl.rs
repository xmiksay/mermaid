//! State-declaration parsing: id/label/class extraction, the `"desc" as id`
//! aliasing form, `[*]`/`[H]` pseudo-state canonicalization, and the shared
//! `ensure_state` upsert.

use std::collections::HashMap;

use crate::parse::{State, StateDiagram, StateKind, Style};

/// Remove a `:::class` token from `raw`, returning the remaining text (with the
/// token excised, so a trailing `: label` survives) and the class name. Only the
/// first occurrence is handled.
pub(super) fn extract_inline_class(raw: &str) -> (String, Option<String>) {
    if let Some(p) = raw.find(":::") {
        let after = &raw[p + 3..];
        let end = after
            .find(|c: char| c.is_whitespace() || c == ':')
            .unwrap_or(after.len());
        let cls = after[..end].to_string();
        let cleaned = format!("{}{}", &raw[..p], &after[end..]);
        let cls = (!cls.is_empty()).then_some(cls);
        (cleaned.trim().to_string(), cls)
    } else {
        (raw.trim().to_string(), None)
    }
}

pub(super) fn apply_state_class(
    diag: &mut StateDiagram,
    existing: &HashMap<String, usize>,
    id: &str,
    class: &str,
) {
    if let Some(&i) = existing.get(id) {
        if !diag.states[i].classes.iter().any(|c| c == class) {
            diag.states[i].classes.push(class.to_string());
        }
    }
}

/// Split a state id off its stereotype marker, mapping the marker to a
/// [`StateKind`]. Handles the `<<choice/fork/join/history>>` stereotype form and
/// the `[[fork]]`/`[[join]]`/`[[choice]]` bracket alternates (upstream lexes the
/// bracket forms as exact aliases of the matching `<<…>>` stereotypes).
pub(super) fn parse_stereotype(id_part: &str) -> (String, StateKind) {
    if let Some(idx) = id_part.find("<<") {
        let id = id_part[..idx].trim().to_string();
        let stereo = id_part[idx + 2..].trim_end_matches(">>").trim();
        let k = match stereo {
            "choice" => StateKind::Choice,
            "fork" => StateKind::Fork,
            "join" => StateKind::Join,
            "history" => StateKind::History { deep: false },
            _ => StateKind::Normal,
        };
        return (id, k);
    }
    if let Some(idx) = id_part.find("[[") {
        let stereo = id_part[idx + 2..].trim_end_matches("]]").trim();
        let k = match stereo {
            "choice" => Some(StateKind::Choice),
            "fork" => Some(StateKind::Fork),
            "join" => Some(StateKind::Join),
            _ => None,
        };
        if let Some(k) = k {
            return (id_part[..idx].trim().to_string(), k);
        }
    }
    (id_part.to_string(), StateKind::Normal)
}

/// Parse the aliasing form `"description" as id`, returning `(id, description)`.
pub(super) fn parse_quoted_as(rest: &str) -> Option<(String, String)> {
    let inner = rest.trim().strip_prefix('"')?;
    let end = inner.find('"')?;
    let desc = inner[..end].to_string();
    let mut after = inner[end + 1..].trim().splitn(2, char::is_whitespace);
    if after.next() != Some("as") {
        return None;
    }
    let id = after.next()?.trim();
    (!id.is_empty()).then(|| (id.to_string(), desc))
}

pub(super) fn parse_state_decl(
    rest: &str,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
) {
    // `:::class` binds tighter than a `: label`, so strip it first.
    let (rest, inline_class) = extract_inline_class(rest.trim());
    if let Some((id, desc)) = parse_quoted_as(&rest) {
        ensure_state(diag, existing, &id, &desc, StateKind::Normal);
        if let Some(cls) = inline_class {
            apply_state_class(diag, existing, &id, &cls);
        }
        return;
    }
    let (id_part, label_part) = match rest.split_once(':') {
        Some((a, b)) => (a.trim(), b.trim().to_string()),
        None => (rest.as_str(), String::new()),
    };
    let (id, kind) = parse_stereotype(id_part);
    ensure_state(diag, existing, &id, &label_part, kind);
    if let Some(cls) = inline_class {
        apply_state_class(diag, existing, &id, &cls);
    }
}

pub(super) fn canonicalize(
    raw: &str,
    is_source: bool,
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
    start_n: &mut usize,
    end_n: &mut usize,
    hist_n: &mut usize,
) -> String {
    if raw == "[*]" {
        if is_source {
            *start_n += 1;
            let id = format!("__start_{start_n}");
            push_pseudo(diag, existing, &id, StateKind::Start);
            id
        } else {
            *end_n += 1;
            let id = format!("__end_{end_n}");
            push_pseudo(diag, existing, &id, StateKind::End);
            id
        }
    } else if raw == "[H]" || raw == "[H*]" {
        let deep = raw == "[H*]";
        *hist_n += 1;
        let id = format!("__hist_{hist_n}");
        push_pseudo(diag, existing, &id, StateKind::History { deep });
        id
    } else {
        ensure_state(diag, existing, raw, "", StateKind::Normal);
        raw.to_string()
    }
}

/// Push a synthesized pseudo-state (`__start_N`/`__end_N`/`__hist_N`) and record
/// it in `existing` so composite region-tracking (which keys off `existing`)
/// counts it as a member — otherwise inner start/end/history circles land
/// outside the composite frame.
fn push_pseudo(
    diag: &mut StateDiagram,
    existing: &mut HashMap<String, usize>,
    id: &str,
    kind: StateKind,
) {
    existing.insert(id.to_string(), diag.states.len());
    diag.states.push(State {
        id: id.to_string(),
        label: String::new(),
        kind,
        classes: Vec::new(),
        style: Style::new(),
        click: None,
    });
}

pub(super) fn ensure_state(
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
        classes: Vec::new(),
        style: Style::new(),
        click: None,
    });
}
