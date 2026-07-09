//! Non-message statements: `autonumber`, notes, actor menus/metadata, and
//! participant declarations (including the v11.12+ `@{ … }` type block).

use crate::parse::ast::{
    AutoNumberConfig, NotePosition, Participant, ParticipantKind, SequenceNote,
};
use crate::parse::token::parse_attr_pairs;
use crate::parse::ParseError;

/// `autonumber` / `autonumber <start>` / `autonumber <start> <step>` /
/// `autonumber off`. Bare `autonumber` starts at 1 with step 1; `off` returns
/// `None`. Non-numeric params fall back to the defaults.
pub(super) fn parse_autonumber(line: &str) -> Option<AutoNumberConfig> {
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

pub(super) fn parse_note(line: &str) -> Option<SequenceNote> {
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
pub(super) fn is_actor_menu(line: &str) -> bool {
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

pub(super) fn parse_participant(
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
