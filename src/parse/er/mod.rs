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

mod entity;
mod relation;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use entity::{is_entity_decl, parse_attribute};
use relation::parse_relation;

use super::ast::{Entity, ErDiagram, FlowDirection};
use super::style::{parse_multi_id_stmt, parse_style_props};
use super::token::split_id_label;
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
            let (names, props) =
                parse_multi_id_stmt(rest, false).ok_or_else(|| malformed("classDef", line_no))?;
            let style = parse_style_props(props);
            for name in names {
                diag.class_defs.insert(name, style.clone());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let (ids, class_name) =
                parse_multi_id_stmt(rest, true).ok_or_else(|| malformed("class", line_no))?;
            for id in ids {
                add_class(&mut diag, &mut by_name, &id, class_name);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("style ") {
            // `style A,B fill:#f9f` — the id side is a comma-separated list.
            let (ids, props) =
                parse_multi_id_stmt(rest, false).ok_or_else(|| malformed("style", line_no))?;
            let style = parse_style_props(props);
            for id in ids {
                let i = entity_index(&mut diag, &mut by_name, &id);
                diag.entities[i].style = style.clone();
            }
            continue;
        }

        // Entity block opener: `NAME {` (or `id[Alias] {`, `NAME:::class {`).
        if let Some(head) = line.strip_suffix('{') {
            let (base, class) = split_style_class(head.trim());
            let (name, label) = split_id_label(base);
            ensure_entity(&mut diag, &mut by_name, &name, Some(&label));
            if let Some(c) = class {
                add_class(&mut diag, &mut by_name, &name, c);
            }
            current_entity = Some(name);
            continue;
        }

        // Relation line: `LEFT <card><line><card> RIGHT : label`
        if let Some((rel, classes)) = parse_relation(line) {
            ensure_entity(&mut diag, &mut by_name, &rel.left, None);
            ensure_entity(&mut diag, &mut by_name, &rel.right, None);
            for (id, class) in &classes {
                add_class(&mut diag, &mut by_name, id, class);
            }
            diag.relations.push(rel);
            continue;
        }

        // Bare entity declaration: `NAME`, `id[Alias]`, or `NAME:::class`.
        if is_entity_decl(line) {
            let (base, class) = split_style_class(line);
            let (name, label) = split_id_label(base);
            ensure_entity(&mut diag, &mut by_name, &name, Some(&label));
            if let Some(c) = class {
                add_class(&mut diag, &mut by_name, &name, c);
            }
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

/// Split a `:::className` style-separator suffix off an entity id.
/// `A:::hot` → (`A`, Some(`hot`)); no separator → (s, None).
fn split_style_class(s: &str) -> (&str, Option<&str>) {
    match s.split_once(":::") {
        Some((id, class)) => {
            let class = class.trim();
            (id.trim(), (!class.is_empty()).then_some(class))
        }
        None => (s.trim(), None),
    }
}

/// Record `class_name` on the entity `id`, materializing a placeholder if the
/// id is not yet declared. Deduplicates so repeated refs don't stack.
fn add_class(
    diag: &mut ErDiagram,
    by_name: &mut HashMap<String, usize>,
    id: &str,
    class_name: &str,
) {
    let i = entity_index(diag, by_name, id);
    if !diag.entities[i].classes.iter().any(|c| c == class_name) {
        diag.entities[i].classes.push(class_name.to_string());
    }
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
    ParseError::malformed(line_no, format!("malformed '{keyword}' statement"))
}
