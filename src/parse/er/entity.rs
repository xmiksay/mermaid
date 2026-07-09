//! Entity-block and bare-declaration parsing for the ER diagram parser.

use crate::parse::ast::EntityAttribute;
use crate::parse::token::unquote;

use super::split_style_class;

pub(super) fn parse_attribute(line: &str) -> EntityAttribute {
    // Trailing "comment" may be a quoted string after the key (or after the name).
    let (head, comment) = if let Some(start) = line.find('"') {
        if let Some(end_rel) = line[start + 1..].find('"') {
            let end = start + 1 + end_rel;
            let comment = line[start + 1..end].to_string();
            let head = format!("{}{}", &line[..start], &line[end + 1..]);
            (head, Some(comment))
        } else {
            (line.to_string(), None)
        }
    } else {
        (line.to_string(), None)
    };
    let mut parts = head.split_whitespace();
    let ty = parts.next().unwrap_or("").to_string();
    let name = parts.next().unwrap_or("").to_string();
    // Remaining tokens are key constraints; upstream allows several
    // (`id PK, FK`), comma- or space-separated. Normalize to `PK, FK`.
    let keys: Vec<String> = parts
        .flat_map(|tok| tok.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let key = if keys.is_empty() {
        None
    } else {
        Some(keys.join(", "))
    };
    EntityAttribute {
        type_: ty,
        name,
        key,
        comment,
    }
}

/// A standalone entity declaration is a single identifier, optionally with an
/// `[Alias]` suffix and/or a `:::class` style separator — no relation
/// connector, no attribute block. A fully quoted name (which may contain
/// spaces) also counts.
pub(super) fn is_entity_decl(line: &str) -> bool {
    let (base, _class) = split_style_class(line);
    // A quoted name may hold spaces upstream refuses in a bare identifier.
    if unquote(base) != base {
        return true;
    }
    let id = match base.split_once('[') {
        Some((id, rest)) => {
            if !rest.ends_with(']') {
                return false;
            }
            id.trim()
        }
        None => base,
    };
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}
