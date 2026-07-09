//! Message-line parsing: arrow-token detection, the `->>+`/`-->>-` activation
//! shorthand, and implicit-participant registration.

use crate::parse::ast::{ArrowKind, Message, Participant, ParticipantKind, SequenceDiagram};
use crate::parse::ParseError;

const ARROWS: &[(&str, ArrowKind)] = &[
    ("<<-->>", ArrowKind::BiDashedArrow),
    ("<<->>", ArrowKind::BiSolidArrow),
    ("-->>", ArrowKind::DashedArrow),
    ("-->", ArrowKind::Dashed),
    ("--x", ArrowKind::DashedCross),
    ("--)", ArrowKind::DashedOpen),
    // v11.12.3+ half (single-barb) arrows, matching upstream
    // sequenceDiagram.jison spellings (#223). The barb is a *doubled* char —
    // `\\` (upper) or `//` (lower) — or a single barb behind a `|` shaft
    // (`|\`/`|/`). Dashed forms carry the extra dash on the shaft side, and the
    // eight reverse forms put the barb at the tail. Longest tokens first so the
    // dashed/pipe variants win over their solid/bare prefixes.
    ("--|\\", ArrowKind::DashedHalfArrowTop),
    ("--|/", ArrowKind::DashedHalfArrowBottom),
    ("--\\\\", ArrowKind::DashedHalfArrowTop),
    ("--//", ArrowKind::DashedHalfArrowBottom),
    ("\\|--", ArrowKind::DashedHalfArrowStartTop),
    ("/|--", ArrowKind::DashedHalfArrowStartBottom),
    ("\\\\--", ArrowKind::DashedHalfArrowStartTop),
    ("//--", ArrowKind::DashedHalfArrowStartBottom),
    ("->>", ArrowKind::SolidArrow),
    ("->", ArrowKind::Solid),
    ("-x", ArrowKind::Cross),
    ("-)", ArrowKind::Open),
    ("-|\\", ArrowKind::HalfArrowTop),
    ("-|/", ArrowKind::HalfArrowBottom),
    ("-\\\\", ArrowKind::HalfArrowTop),
    ("-//", ArrowKind::HalfArrowBottom),
    ("\\|-", ArrowKind::HalfArrowStartTop),
    ("/|-", ArrowKind::HalfArrowStartBottom),
    ("\\\\-", ArrowKind::HalfArrowStartTop),
    ("//-", ArrowKind::HalfArrowStartBottom),
];

/// Activation shorthand attached to a message arrow (`->>+` / `-->>-`).
pub(super) enum Activation {
    None,
    Activate,
    Deactivate,
}

pub(super) fn parse_message(
    line: &str,
    line_no: usize,
) -> Result<(Message, Activation), ParseError> {
    let (arrow_pos, token, kind) = find_arrow(line).ok_or_else(|| {
        ParseError::unknown(line_no, format!("not a recognized statement: '{line}'"))
    })?;
    let from = line[..arrow_pos].trim().to_string();
    if from.is_empty() {
        return Err(ParseError::malformed(line_no, "empty sender"));
    }
    let after = &line[arrow_pos + token.len()..];
    let (target_part, text) = match after.find(':') {
        Some(c) => (after[..c].trim(), after[c + 1..].trim().to_string()),
        None => (after.trim(), String::new()),
    };
    // A leading `+`/`-` on the target is the activation shorthand, not part of
    // the participant id.
    let (activation, target_part) = match target_part.strip_prefix('+') {
        Some(rest) => (Activation::Activate, rest.trim_start()),
        None => match target_part.strip_prefix('-') {
            Some(rest) => (Activation::Deactivate, rest.trim_start()),
            None => (Activation::None, target_part),
        },
    };
    let to = target_part.to_string();
    if to.is_empty() {
        return Err(ParseError::malformed(line_no, "empty receiver"));
    }
    Ok((
        Message {
            from,
            to,
            text,
            arrow: kind,
        },
        activation,
    ))
}

fn find_arrow(line: &str) -> Option<(usize, &'static str, ArrowKind)> {
    let mut best: Option<(usize, &'static str, ArrowKind)> = None;
    for &(tok, kind) in ARROWS {
        if let Some(pos) = line.find(tok) {
            match best {
                Some((p, _, _)) if p < pos => {}
                Some((p, t, _)) if p == pos && t.len() >= tok.len() => {}
                _ => best = Some((pos, tok, kind)),
            }
        }
    }
    best
}

pub(super) fn register_implicit_participant(diag: &mut SequenceDiagram, id: &str) {
    if diag.participants.iter().any(|p| p.id == id) {
        return;
    }
    diag.participants.push(Participant {
        id: id.to_string(),
        display: id.to_string(),
        kind: ParticipantKind::Participant,
    });
}
