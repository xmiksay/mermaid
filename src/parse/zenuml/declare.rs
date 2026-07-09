//! Participant declarations: bare/alias declarations, `@`-annotator
//! stereotypes, `@Starter(...)`, and the lazy `ensure`/`declare`/`source`
//! bookkeeping that materializes participants as they are first referenced.

use super::super::ast::{Participant, ParticipantKind};
use super::{Parser, DEFAULT_STARTER};

impl Parser {
    /// A participant declaration: a bare identifier (`Bob`) or an alias
    /// (`A as Alice`, id `A` displayed as `Alice`). Returns `false` for anything
    /// that carries a call (`(`) or an arrow (`->`), leaving it to `parse_call`.
    pub(super) fn try_declaration(&mut self, s: &str) -> bool {
        let s = s.trim();
        if s.contains('(') || s.contains("->") {
            return false;
        }
        if let Some((id, display)) = split_alias(s) {
            self.declare_alias(&id, &display);
            return true;
        }
        if !s.is_empty() && is_identifier(s) {
            self.ensure(s);
            return true;
        }
        false
    }

    fn declare_alias(&mut self, id: &str, display: &str) {
        if let Some(p) = self.diag.participants.iter_mut().find(|p| p.id == id) {
            p.display = display.to_string();
        } else {
            self.diag.participants.push(Participant {
                id: id.to_string(),
                display: display.to_string(),
                kind: ParticipantKind::Participant,
            });
        }
    }

    /// Declare a participant from an annotator line (`Actor Alice`,
    /// `Database DB`, `Starter(Alice)`).
    pub(super) fn annotator(&mut self, rest: &str) {
        let rest = rest.trim();
        if let Some(inner) = rest
            .strip_prefix("Starter(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let id = inner.trim().to_string();
            if !id.is_empty() {
                self.ensure(&id);
                self.starter = Some(id);
            }
            return;
        }
        let (kind_word, name) = match rest.split_once(char::is_whitespace) {
            Some((k, n)) => (k, n.trim()),
            None => return, // `@Type` with no name declares nothing.
        };
        if name.is_empty() {
            return;
        }
        let kind = match kind_word.to_ascii_lowercase().as_str() {
            "actor" => ParticipantKind::Actor,
            "boundary" => ParticipantKind::Boundary,
            "control" => ParticipantKind::Control,
            "entity" => ParticipantKind::Entity,
            "database" => ParticipantKind::Database,
            _ => ParticipantKind::Participant,
        };
        self.declare(name, kind);
    }

    /// The originating participant for a context: the enclosing receiver, or the
    /// starter (created lazily on first top-level use).
    pub(super) fn source(&mut self, ctx: Option<&str>) -> String {
        match ctx {
            Some(c) => c.to_string(),
            None => {
                let id = self
                    .starter
                    .clone()
                    .unwrap_or_else(|| DEFAULT_STARTER.into());
                self.ensure(&id);
                self.starter = Some(id.clone());
                id
            }
        }
    }

    pub(super) fn ensure(&mut self, id: &str) {
        if !self.diag.participants.iter().any(|p| p.id == id) {
            self.diag.participants.push(Participant {
                id: id.to_string(),
                display: id.to_string(),
                kind: ParticipantKind::Participant,
            });
        }
    }

    fn declare(&mut self, id: &str, kind: ParticipantKind) {
        if let Some(p) = self.diag.participants.iter_mut().find(|p| p.id == id) {
            p.kind = kind;
        } else {
            self.diag.participants.push(Participant {
                id: id.to_string(),
                display: id.to_string(),
                kind,
            });
        }
    }
}

/// True if `s` is a single participant identifier (letters, digits, `_`).
fn is_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Split an `Id as Display` alias declaration into `(id, display)`. `id` must be
/// a plain identifier; `display` may be quoted. Returns `None` when the `as`
/// keyword is absent.
fn split_alias(s: &str) -> Option<(String, String)> {
    let (id, rest) = s.trim().split_once(char::is_whitespace)?;
    let after = rest.trim_start().strip_prefix("as")?;
    // `as` must be a whole word, not the head of a longer identifier.
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let display = after.trim().trim_matches('"').trim();
    if !is_identifier(id) || display.is_empty() {
        return None;
    }
    Some((id.to_string(), display.to_string()))
}
