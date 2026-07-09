//! Per-line statement parsing: messages (with the `->>+`/`-->>-` activation
//! shorthand), participants, notes, autonumber, and actor menus.

mod arrow;
mod participant;
#[cfg(test)]
mod tests;

use crate::parse::ast::{ParticipantKind, SequenceDiagram, SequenceItem};
use crate::parse::ParseError;

use arrow::{parse_message, register_implicit_participant, Activation};
use participant::{is_actor_menu, parse_autonumber, parse_note, parse_participant};

pub(super) fn parse_line_to_items(
    line: &str,
    diag: &mut SequenceDiagram,
    line_no: usize,
) -> Result<Vec<SequenceItem>, ParseError> {
    // Both the space form (`title Demo`) and the legacy colon form
    // (`title: Demo`, upstream lexer `"title:"\s[^#\n;]+`).
    if let Some(rest) = line
        .strip_prefix("title ")
        .or_else(|| line.strip_prefix("title:"))
    {
        diag.title = Some(rest.trim().to_string());
        return Ok(Vec::new());
    }
    if line == "autonumber" || line.starts_with("autonumber ") {
        let cfg = parse_autonumber(line);
        if cfg.is_some() {
            diag.autonumber = true;
        }
        return Ok(vec![SequenceItem::AutoNumber(cfg)]);
    }

    if let Some(rest) = line.strip_prefix("participant ") {
        let p = parse_participant(rest, ParticipantKind::Participant, line_no)?;
        diag.participants.push(p);
        return Ok(Vec::new());
    }
    if let Some(rest) = line.strip_prefix("actor ") {
        let p = parse_participant(rest, ParticipantKind::Actor, line_no)?;
        diag.participants.push(p);
        return Ok(Vec::new());
    }

    if let Some(rest) = line.strip_prefix("create ") {
        let (kind, decl) = match (
            rest.trim().strip_prefix("participant "),
            rest.trim().strip_prefix("actor "),
        ) {
            (Some(r), _) => (ParticipantKind::Participant, r),
            (_, Some(r)) => (ParticipantKind::Actor, r),
            _ => (ParticipantKind::Participant, rest.trim()),
        };
        let p = parse_participant(decl, kind, line_no)?;
        let id = p.id.clone();
        diag.participants.push(p);
        return Ok(vec![SequenceItem::Create(id)]);
    }
    if let Some(rest) = line.strip_prefix("destroy ") {
        return Ok(vec![SequenceItem::Destroy(rest.trim().to_string())]);
    }

    // Actor metadata (`link A: Label @ url`, `links A: {json}`,
    // `properties A: {json}`, `details A: {json}`) is consumed but not rendered
    // — accepting the syntax keeps it from being a hard error.
    if is_actor_menu(line) {
        return Ok(Vec::new());
    }

    if let Some(rest) = line.strip_prefix("activate ") {
        return Ok(vec![SequenceItem::Activate(rest.trim().to_string())]);
    }
    if let Some(rest) = line.strip_prefix("deactivate ") {
        return Ok(vec![SequenceItem::Deactivate(rest.trim().to_string())]);
    }

    if let Some(note) = parse_note(line) {
        return Ok(vec![SequenceItem::Note(note)]);
    }

    let (msg, activation) = parse_message(line, line_no)?;
    register_implicit_participant(diag, &msg.from);
    register_implicit_participant(diag, &msg.to);
    // Activation shorthand, both *after* the message (upstream jison
    // `actor signaltype +/- actor text`): `->>+B` activates the receiver
    // (`msg.to`), `-->>-B` deactivates the *sender* (`msg.from`) — the
    // participant that was activated when it earlier received a message.
    let receiver = msg.to.clone();
    let sender = msg.from.clone();
    let mut items = Vec::new();
    match activation {
        Activation::Activate => {
            items.push(SequenceItem::Message(msg));
            items.push(SequenceItem::Activate(receiver));
        }
        Activation::Deactivate => {
            items.push(SequenceItem::Message(msg));
            items.push(SequenceItem::Deactivate(sender));
        }
        Activation::None => items.push(SequenceItem::Message(msg)),
    }
    Ok(items)
}
