//! ER diagram parser (v0.1 subset).
//!
//! Supports:
//!   * `erDiagram` header.
//!   * Entity blocks: `ENTITY { type name [PK|FK|UK] ... }`.
//!   * Relations with Crow's Foot cardinality:
//!     `LEFT <leftCard><line><rightCard> RIGHT : label`
//!     where `<line>` is `--` (identifying) or `..` (non-identifying).

use std::collections::HashMap;

use crate::ast::{Cardinality, Entity, EntityAttribute, ErDiagram, ErRelation};
use crate::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<ErDiagram, ParseError> {
    let mut diag = ErDiagram::default();
    let mut header_seen = false;
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut current_entity: Option<String> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "erDiagram" {
                return Err(ParseError::Syntax {
                    message: "expected 'erDiagram' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(entity_name) = current_entity.clone() {
            if line == "}" {
                current_entity = None;
                continue;
            }
            let attr = parse_attribute(line);
            let i = by_name[&entity_name];
            diag.entities[i].attributes.push(attr);
            continue;
        }

        // Entity block opener: `NAME {`
        if let Some(name) = line.strip_suffix('{') {
            let name = name.trim().to_string();
            ensure_entity(&mut diag, &mut by_name, &name);
            current_entity = Some(name);
            continue;
        }

        // Relation line: `LEFT <card><line><card> RIGHT : label`
        if let Some(rel) = parse_relation(line) {
            ensure_entity(&mut diag, &mut by_name, &rel.left);
            ensure_entity(&mut diag, &mut by_name, &rel.right);
            diag.relations.push(rel);
            continue;
        }

        return Err(ParseError::Syntax {
            message: format!("unrecognized ER statement: '{line}'"),
            line: line_no,
        });
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

fn parse_attribute(line: &str) -> EntityAttribute {
    let mut parts = line.split_whitespace();
    let ty = parts.next().unwrap_or("").to_string();
    let name = parts.next().unwrap_or("").to_string();
    let key = parts.next().map(|k| k.to_string());
    EntityAttribute {
        type_: ty,
        name,
        key,
    }
}

fn parse_relation(line: &str) -> Option<ErRelation> {
    // Find the connector substring. It's surrounded by spaces (typical) but we
    // scan for the line separator (`--` or `..`) anywhere.
    let line_styles = ["--", ".."];
    let (line_start, line_style) = line_styles
        .iter()
        .filter_map(|s| line.find(s).map(|p| (p, *s)))
        .min_by_key(|x| x.0)?;

    // The cardinality token sits immediately before `line_start`. Strip any
    // trailing whitespace from the left-of-line text.
    let left_part = &line[..line_start];
    let left_trim_end = left_part.trim_end_matches(' ');
    // The token can be 1 or 2 chars: `||`, `o|`, `|{`, `o{`, then mirrored
    // versions on the other side. We match longest first.
    let (left_card_len, left_card) = scan_card_end(left_trim_end)?;
    let left_name = left_trim_end[..left_trim_end.len() - left_card_len]
        .trim()
        .to_string();

    let after_line = &line[line_start + line_style.len()..];
    let (right_card_len, right_card) = scan_card_start(after_line)?;
    let after_card = &after_line[right_card_len..];

    // Remaining: ` RIGHT : label`
    let after_card = after_card.trim_start();
    let (right_name, label) = match after_card.split_once(':') {
        Some((a, b)) => (a.trim().to_string(), b.trim().trim_matches('"').to_string()),
        None => (after_card.trim().to_string(), String::new()),
    };
    if right_name.is_empty() {
        return None;
    }

    Some(ErRelation {
        left: left_name,
        right: right_name,
        left_card,
        right_card,
        identifying: line_style == "--",
        label,
    })
}

/// Match a cardinality token at the END of `s`. Returns (length, card).
fn scan_card_end(s: &str) -> Option<(usize, Cardinality)> {
    // Left-side tokens (incoming line direction): `||`, `|o`, `}|`, `}o`
    // These are mirrored versions of the right-side tokens.
    const ENDS: &[(&str, Cardinality)] = &[
        ("}o", Cardinality::ZeroOrMore),
        ("}|", Cardinality::OneOrMore),
        ("o|", Cardinality::ZeroOrOne),
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
    ];
    for (tok, card) in ENDS {
        if s.ends_with(tok) {
            return Some((tok.len(), *card));
        }
    }
    None
}

/// Match a cardinality token at the START of `s`. Returns (length, card).
fn scan_card_start(s: &str) -> Option<(usize, Cardinality)> {
    const STARTS: &[(&str, Cardinality)] = &[
        ("o{", Cardinality::ZeroOrMore),
        ("|{", Cardinality::OneOrMore),
        ("o|", Cardinality::ZeroOrOne),
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
    ];
    for (tok, card) in STARTS {
        if s.starts_with(tok) {
            return Some((tok.len(), *card));
        }
    }
    None
}

fn ensure_entity(diag: &mut ErDiagram, by_name: &mut HashMap<String, usize>, name: &str) {
    if by_name.contains_key(name) {
        return;
    }
    by_name.insert(name.to_string(), diag.entities.len());
    diag.entities.push(Entity {
        name: name.to_string(),
        attributes: Vec::new(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relations_basic() {
        let s = "erDiagram\nCUSTOMER ||--o{ ORDER : places\nORDER ||--|{ LINE-ITEM : contains\n";
        let d = parse(s).unwrap();
        assert_eq!(d.relations.len(), 2);
        let r0 = &d.relations[0];
        assert_eq!(r0.left, "CUSTOMER");
        assert_eq!(r0.right, "ORDER");
        assert_eq!(r0.left_card, Cardinality::ExactlyOne);
        assert_eq!(r0.right_card, Cardinality::ZeroOrMore);
        assert_eq!(r0.label, "places");
        assert!(r0.identifying);
    }

    #[test]
    fn dotted_line_is_nonidentifying() {
        let s = "erDiagram\nA }|..|{ B : uses\n";
        let d = parse(s).unwrap();
        assert!(!d.relations[0].identifying);
        assert_eq!(d.relations[0].left_card, Cardinality::OneOrMore);
        assert_eq!(d.relations[0].right_card, Cardinality::OneOrMore);
    }

    #[test]
    fn entity_block() {
        let s = "erDiagram\nCUSTOMER {\nstring name\nstring email PK\n}\nCUSTOMER ||--o{ ORDER : places\n";
        let d = parse(s).unwrap();
        let c = d.entities.iter().find(|e| e.name == "CUSTOMER").unwrap();
        assert_eq!(c.attributes.len(), 2);
        assert_eq!(c.attributes[1].key.as_deref(), Some("PK"));
    }
}
