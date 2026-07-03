//! ER diagram parser.
//!
//! Supports:
//!   * `erDiagram` header and `direction TB/BT/LR/RL`.
//!   * Entity blocks `ENTITY { type name [PK[, FK ...]] "comment" }` and the
//!     alias form `id[Alias] { ... }` (id stays clean, Alias is the label).
//!   * Relations with Crow's Foot cardinality:
//!     `LEFT <leftCard><line><rightCard> RIGHT : label`
//!     where `<line>` is `--` (identifying) or `..` (non-identifying), plus the
//!     verbal form `LEFT <card> to|optionally to <card> RIGHT : label` with
//!     word/numeric cardinalities (`only one`, `zero or more`, `one or many`,
//!     `0+`, `1`, …).

use std::collections::HashMap;

use super::ast::{Cardinality, Entity, EntityAttribute, ErDiagram, ErRelation, FlowDirection};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

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
                return Err(ParseError::header(line_no, "expected 'erDiagram' header"));
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

        // `direction TB/BT/LR/RL` — drives the layout transpose.
        if let Some(rest) = line.strip_prefix("direction ") {
            diag.direction = match rest.trim() {
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

        // Styling directives (upstream `erDiagram` grammar): `classDef`,
        // `class`, `style` — shared with the flowchart's resolve_style path.
        if let Some(rest) = line.strip_prefix("classDef ") {
            let (names, props) = rest
                .trim()
                .split_once(char::is_whitespace)
                .ok_or_else(|| malformed("classDef", line_no))?;
            let style = parse_style_props(props);
            for name in names.split(',') {
                let name = name.trim();
                if !name.is_empty() {
                    diag.class_defs.insert(name.to_string(), style.clone());
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let (ids, class_name) = rest
                .trim()
                .rsplit_once(char::is_whitespace)
                .ok_or_else(|| malformed("class", line_no))?;
            let class_name = class_name.trim();
            if class_name.is_empty() {
                return Err(malformed("class", line_no));
            }
            for id in ids.split(',') {
                let id = id.trim();
                if id.is_empty() {
                    continue;
                }
                let i = entity_index(&mut diag, &mut by_name, id);
                if !diag.entities[i].classes.iter().any(|c| c == class_name) {
                    diag.entities[i].classes.push(class_name.to_string());
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("style ") {
            let (id, props) = rest
                .trim()
                .split_once(char::is_whitespace)
                .ok_or_else(|| malformed("style", line_no))?;
            let i = entity_index(&mut diag, &mut by_name, id.trim());
            diag.entities[i].style = parse_style_props(props);
            continue;
        }

        // Entity block opener: `NAME {` (or `id[Alias] {`).
        if let Some(head) = line.strip_suffix('{') {
            let (name, label) = split_id_label(head.trim());
            ensure_entity(&mut diag, &mut by_name, &name, Some(&label));
            current_entity = Some(name);
            continue;
        }

        // Relation line: `LEFT <card><line><card> RIGHT : label`
        if let Some(rel) = parse_relation(line) {
            ensure_entity(&mut diag, &mut by_name, &rel.left, None);
            ensure_entity(&mut diag, &mut by_name, &rel.right, None);
            diag.relations.push(rel);
            continue;
        }

        // Bare entity declaration: `NAME` or `id[Alias]`.
        if is_entity_decl(line) {
            let (name, label) = split_id_label(line);
            ensure_entity(&mut diag, &mut by_name, &name, Some(&label));
            continue;
        }

        return Err(ParseError::unknown(
            line_no,
            format!("unrecognized ER statement: '{line}'"),
        ));
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

/// A standalone entity declaration is a single identifier, optionally with an
/// `[Alias]` suffix — no relation connector, no attribute block.
fn is_entity_decl(line: &str) -> bool {
    let id = match line.split_once('[') {
        Some((id, rest)) => {
            if !rest.ends_with(']') {
                return false;
            }
            id.trim()
        }
        None => line,
    };
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Split an `id[Alias]` form into `(id, label)`; a plain id reuses itself as
/// the label. Mirrors the flowchart node-label split.
fn split_id_label(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(open) = s.find('[') {
        if s.ends_with(']') {
            let id = s[..open].trim();
            let label = s[open + 1..s.len() - 1].trim();
            let id = if id.is_empty() { label } else { id };
            return (id.to_string(), label.to_string());
        }
    }
    (s.to_string(), s.to_string())
}

fn parse_attribute(line: &str) -> EntityAttribute {
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

fn parse_relation(line: &str) -> Option<ErRelation> {
    // Split off the trailing `: label` first (entity ids never contain `:`).
    let (spec, label) = match line.split_once(':') {
        Some((a, b)) => (a.trim(), b.trim().trim_matches('"').to_string()),
        None => (line.trim(), String::new()),
    };

    // Locate the relationship connector between the two cardinalities. It is
    // either a glyph line (`--`/`..`) or a verbal form (`to`/`optionally to`).
    let (sep_start, sep_len, identifying) = find_reltype(spec)?;
    let left_part = spec[..sep_start].trim();
    let right_part = spec[sep_start + sep_len..].trim();

    let (left_name, left_card) = split_card_end(left_part)?;
    let (right_card, right_name) = split_card_start(right_part)?;
    if left_name.is_empty() || right_name.is_empty() {
        return None;
    }

    Some(ErRelation {
        left: split_id_label(left_name).0,
        right: split_id_label(right_name).0,
        left_card,
        right_card,
        identifying,
        label,
    })
}

/// Locate the relationship connector, returning `(byte offset, length,
/// identifying)`. Verbal forms take precedence over the glyph lines so an
/// `optionally to` isn't mistaken for a bare `to`.
fn find_reltype(spec: &str) -> Option<(usize, usize, bool)> {
    const VERBAL: &[(&str, bool)] = &[(" optionally to ", false), (" to ", true)];
    for (tok, identifying) in VERBAL {
        if let Some(p) = spec.find(tok) {
            return Some((p, tok.len(), *identifying));
        }
    }
    ["--", ".."]
        .iter()
        .filter_map(|s| spec.find(s).map(|p| (p, *s)))
        .min_by_key(|x| x.0)
        .map(|(p, s)| (p, s.len(), s == "--"))
}

/// Verbal / numeric cardinality aliases (upstream `erDiagram` grammar), matched
/// as whole words. Glyph tokens are handled separately.
const CARD_WORDS: &[(&str, Cardinality)] = &[
    ("only one", Cardinality::ExactlyOne),
    ("zero or one", Cardinality::ZeroOrOne),
    ("one or zero", Cardinality::ZeroOrOne),
    ("zero or more", Cardinality::ZeroOrMore),
    ("zero or many", Cardinality::ZeroOrMore),
    ("one or more", Cardinality::OneOrMore),
    ("one or many", Cardinality::OneOrMore),
    ("many(1)", Cardinality::OneOrMore),
    ("many(0)", Cardinality::ZeroOrMore),
    ("0+", Cardinality::ZeroOrMore),
    ("1+", Cardinality::OneOrMore),
    ("1", Cardinality::ExactlyOne),
];

/// Split `NAME <card>` where the cardinality sits at the END. Handles verbal /
/// numeric words as well as the crow's-foot glyphs.
fn split_card_end(s: &str) -> Option<(&str, Cardinality)> {
    let s = s.trim_end();
    for (tok, card) in CARD_WORDS {
        if let Some(name) = s.strip_suffix(tok) {
            if name.is_empty() || name.ends_with(char::is_whitespace) {
                return Some((name.trim_end(), *card));
            }
        }
    }
    const GLYPHS: &[(&str, Cardinality)] = &[
        ("}o", Cardinality::ZeroOrMore),
        ("}|", Cardinality::OneOrMore),
        ("o|", Cardinality::ZeroOrOne),
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
    ];
    for (tok, card) in GLYPHS {
        if let Some(name) = s.strip_suffix(tok) {
            return Some((name.trim_end(), *card));
        }
    }
    None
}

/// Split `<card> NAME` where the cardinality sits at the START.
fn split_card_start(s: &str) -> Option<(Cardinality, &str)> {
    let s = s.trim_start();
    for (tok, card) in CARD_WORDS {
        if let Some(name) = s.strip_prefix(tok) {
            if name.is_empty() || name.starts_with(char::is_whitespace) {
                return Some((*card, name.trim_start()));
            }
        }
    }
    const GLYPHS: &[(&str, Cardinality)] = &[
        ("o{", Cardinality::ZeroOrMore),
        ("|{", Cardinality::OneOrMore),
        ("o|", Cardinality::ZeroOrOne),
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
    ];
    for (tok, card) in GLYPHS {
        if let Some(name) = s.strip_prefix(tok) {
            return Some((*card, name.trim_start()));
        }
    }
    None
}

fn ensure_entity(
    diag: &mut ErDiagram,
    by_name: &mut HashMap<String, usize>,
    name: &str,
    label: Option<&str>,
) {
    if let Some(&i) = by_name.get(name) {
        // A later `id[Alias]` (or block opener) upgrades a placeholder label
        // materialized by an earlier relation reference.
        if let Some(l) = label {
            if diag.entities[i].label == diag.entities[i].name {
                diag.entities[i].label = l.to_string();
            }
        }
        return;
    }
    by_name.insert(name.to_string(), diag.entities.len());
    diag.entities.push(Entity {
        name: name.to_string(),
        label: label.unwrap_or(name).to_string(),
        attributes: Vec::new(),
        classes: Vec::new(),
        style: Vec::new(),
    });
}

/// Index of the entity with `name`, materializing a placeholder if a styling
/// directive references it before it is declared (mirrors the flowchart).
fn entity_index(diag: &mut ErDiagram, by_name: &mut HashMap<String, usize>, name: &str) -> usize {
    ensure_entity(diag, by_name, name, None);
    by_name[name]
}

/// A `ParseError::Syntax` for a recognized directive keyword with a malformed body.
fn malformed(keyword: &str, line_no: usize) -> ParseError {
    ParseError::Syntax {
        message: format!("malformed '{keyword}' statement"),
        line: line_no,
    }
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

    #[test]
    fn verbal_cardinalities() {
        let s = "erDiagram\nCAR only one to zero or more NAMED-DRIVER : allows\n";
        let d = parse(s).unwrap();
        let r = &d.relations[0];
        assert_eq!(r.left, "CAR");
        assert_eq!(r.right, "NAMED-DRIVER");
        assert_eq!(r.left_card, Cardinality::ExactlyOne);
        assert_eq!(r.right_card, Cardinality::ZeroOrMore);
        assert!(r.identifying);
        assert_eq!(r.label, "allows");
    }

    #[test]
    fn optionally_to_is_nonidentifying() {
        let d = parse("erDiagram\nA one or many optionally to one or zero B : x\n").unwrap();
        let r = &d.relations[0];
        assert_eq!(r.left_card, Cardinality::OneOrMore);
        assert_eq!(r.right_card, Cardinality::ZeroOrOne);
        assert!(!r.identifying);
    }

    #[test]
    fn numeric_cardinalities() {
        let d = parse("erDiagram\nPERSON 1--1 CAR : owns\n").unwrap();
        let r = &d.relations[0];
        assert_eq!(r.left, "PERSON");
        assert_eq!(r.right, "CAR");
        assert_eq!(r.left_card, Cardinality::ExactlyOne);
        assert_eq!(r.right_card, Cardinality::ExactlyOne);
    }

    #[test]
    fn entity_alias_no_duplicate() {
        let s = "erDiagram\np[Person] {\nstring name\n}\np ||--o{ ORDER : places\n";
        let d = parse(s).unwrap();
        assert_eq!(d.entities.iter().filter(|e| e.name == "p").count(), 1);
        let p = d.entities.iter().find(|e| e.name == "p").unwrap();
        assert_eq!(p.label, "Person");
        assert_eq!(p.attributes.len(), 1);
    }

    #[test]
    fn alias_upgrades_earlier_reference() {
        // Relation references `p` before its aliased block appears.
        let s = "erDiagram\np ||--o{ ORDER : places\np[Person] {\nstring name\n}\n";
        let d = parse(s).unwrap();
        assert_eq!(d.entities.iter().filter(|e| e.name == "p").count(), 1);
        assert_eq!(
            d.entities.iter().find(|e| e.name == "p").unwrap().label,
            "Person"
        );
    }

    #[test]
    fn direction_keyword() {
        let d = parse("erDiagram\ndirection LR\nA ||--o{ B : x\n").unwrap();
        assert_eq!(d.direction, FlowDirection::LeftRight);
    }

    #[test]
    fn multiple_key_constraints() {
        let d = parse("erDiagram\nORDER {\nstring id PK, FK\n}\n").unwrap();
        let o = d.entities.iter().find(|e| e.name == "ORDER").unwrap();
        assert_eq!(o.attributes[0].key.as_deref(), Some("PK, FK"));
    }

    #[test]
    fn classdef_and_class_apply() {
        let s = "erDiagram\nCUSTOMER ||--o{ ORDER : places\nclassDef hot fill:#f00,stroke:#900\nclass CUSTOMER hot\n";
        let d = parse(s).unwrap();
        assert_eq!(d.class_defs.len(), 1);
        assert!(d.class_defs.contains_key("hot"));
        let c = d.entities.iter().find(|e| e.name == "CUSTOMER").unwrap();
        assert_eq!(c.classes, vec!["hot".to_string()]);
        // The other entity carries no class.
        let o = d.entities.iter().find(|e| e.name == "ORDER").unwrap();
        assert!(o.classes.is_empty());
    }

    #[test]
    fn style_directive_on_entity() {
        let d = parse("erDiagram\nORDER {\nstring id\n}\nstyle ORDER fill:#0f0\n").unwrap();
        let o = d.entities.iter().find(|e| e.name == "ORDER").unwrap();
        assert_eq!(o.style, vec![("fill".to_string(), "#0f0".to_string())]);
    }

    #[test]
    fn classdef_without_props_errors() {
        assert!(parse("erDiagram\nA ||--|| B : x\nclassDef foo\n").is_err());
    }

    #[test]
    fn attribute_comment_parsed() {
        let d = parse("erDiagram\nCUSTOMER {\nstring name \"the customer name\"\n}\n").unwrap();
        let c = d.entities.iter().find(|e| e.name == "CUSTOMER").unwrap();
        assert_eq!(
            c.attributes[0].comment.as_deref(),
            Some("the customer name")
        );
    }
}
