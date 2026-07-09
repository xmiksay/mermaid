//! Class diagram parser.
//!
//! Supports:
//!   * Header: `classDiagram` (or the `classDiagram-v2` alias).
//!   * Class blocks: `class X { ... }` with member lines inside.
//!   * Member shorthand: `X : +method()`, `X : -attr int`.
//!   * Stereotype: `class X { <<interface>> ... }` or `X <<abstract>>`.
//!   * Namespaces: `namespace X { class A; class B }`.
//!   * `direction` directive (TD/BT/LR/RL).
//!   * Notes (`note "…"`, `note for X "…"`), standalone annotations
//!     (either order), `Name["label"]`, and `click`/`link`/`callback`.
//!   * Generics `~T~` rendered as `<T>`.
//!   * Relationships:
//!     `<|--` `<|..`  inheritance / realization
//!     `*--`         composition
//!     `o--`         aggregation
//!     `-->`         association (with arrow)
//!     `--`          link
//!     `..`          dashed link
//!     `..>`         dependency
//!     `()--` `--()` lollipop interface (socket circle at the `()` end)
//!     With optional role multiplicities (`A "1" --> "*" B`) and `: label`.

use std::collections::HashMap;

use super::ast::{ClassDiagram, ClassRelation, FlowDirection, Namespace};
use super::token::extract_inline_class;
use super::{strip_comment, ParseError};

mod decl;
mod notes;
mod relation;
#[cfg(test)]
mod tests;

use decl::{
    add_member_line, extract_class_label, get_class, handle_class_decl, handle_class_def,
    handle_css_class, handle_style, parse_member,
};
use notes::{parse_interaction, parse_note, parse_standalone_annotation, split_interaction};
use relation::{
    detect_two_way, find_relation, is_reversed_token, split_leading_card, split_leading_lollipop,
    split_trailing_card, split_trailing_lollipop,
};

pub(crate) fn parse(input: &str) -> Result<ClassDiagram, ParseError> {
    let mut diag = ClassDiagram::default();
    let mut header_seen = false;
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut in_block: Option<String> = None;
    let mut namespace_stack: Vec<String> = Vec::new();

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "classDiagram" && line != "classDiagram-v2" {
                return Err(ParseError::header(
                    line_no,
                    "expected 'classDiagram' header",
                ));
            }
            header_seen = true;
            continue;
        }

        if let Some(d) = line.strip_prefix("direction ") {
            diag.direction = match d.trim() {
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

        if let Some(rest) = line.strip_prefix("namespace ") {
            let inner = rest.trim().trim_end_matches('{').trim();
            let (name, label) = extract_class_label(inner);
            let depth = namespace_stack.len();
            namespace_stack.push(name.clone());
            diag.namespaces.push(Namespace {
                name,
                label,
                depth,
                class_names: Vec::new(),
            });
            continue;
        }

        if line == "}" && in_block.is_none() && !namespace_stack.is_empty() {
            namespace_stack.pop();
            continue;
        }

        if let Some(class_name) = &in_block.clone() {
            if line == "}" {
                in_block = None;
                continue;
            }
            // Member of the open block.
            add_member_line(&mut diag, &mut by_name, class_name, line);
            continue;
        }

        if let Some(rest) = line.strip_prefix("classDef ") {
            handle_class_def(rest, &mut diag);
            continue;
        }
        if let Some(rest) = line.strip_prefix("style ") {
            handle_style(rest, &mut diag, &mut by_name);
            continue;
        }
        if let Some(rest) = line.strip_prefix("cssClass ") {
            handle_css_class(rest, &mut diag, &mut by_name);
            continue;
        }

        if let Some(rest) = line.strip_prefix("class ") {
            let added_name = handle_class_decl(rest, &mut diag, &mut by_name, &mut in_block);
            // Register the class with every namespace on the stack so a nested
            // namespace's classes also count toward its ancestors' frame bounds.
            for ns_name in &namespace_stack {
                if let Some(ns) = diag.namespaces.iter_mut().find(|n| n.name == *ns_name) {
                    if !ns.class_names.contains(&added_name) {
                        ns.class_names.push(added_name.clone());
                    }
                }
            }
            continue;
        }

        // `note "text"` (free) / `note for <Class> "text"` (attached).
        if let Some(rest) = line.strip_prefix("note ") {
            diag.notes.push(parse_note(rest));
            continue;
        }

        // Interactivity: `click`/`link`/`callback` bind a hyperlink or JS
        // callback to a class. Handled before the `:`-shorthand split so a URL's
        // `https://` colon can't route the line down the member path.
        if let Some((kind, rest)) = split_interaction(line) {
            if let Some((name, action)) = parse_interaction(kind, rest) {
                get_class(&mut diag, &mut by_name, &name).click = Some(action);
                continue;
            }
        }

        // Shorthand: "ClassName : member"
        if let Some((cls_name, member_str)) = line.split_once(':') {
            let cls_name = cls_name.trim();
            // Distinguish from relation lines that also contain ':' (label).
            // Relation lines must contain one of the relation tokens.
            if find_relation(cls_name).is_none() && find_relation(line).is_none() {
                let member = parse_member(member_str.trim());
                let cls = get_class(&mut diag, &mut by_name, cls_name);
                cls.members.push(member);
                continue;
            }
        }

        if let Some((tok_pos, tok, kind)) = find_relation(line) {
            let reversed = is_reversed_token(tok);
            // Two-way relations glue a mirror marker (`|>`/`>`/`*`/`o`) onto the
            // token; consume it here so it doesn't leak into the right class.
            let (to_kind, two_way_len) =
                detect_two_way(&line[tok_pos + tok.len()..], tok, reversed);
            let after_tok = tok_pos + tok.len() + two_way_len;
            // Left end: `Class[:::style] ["card"] [()]`. The lollipop `()` sits
            // right against the token, so strip it before the multiplicity.
            let (left, lollipop_from) = split_trailing_lollipop(line[..tok_pos].trim());
            let (left, from_class) = extract_inline_class(&left);
            let (from, from_card) = split_trailing_card(&left);
            // Right end: `[()] ["card"] Class[:::style] [: label]`. Strip the
            // lollipop, then the leading multiplicity and any `:::style` before
            // splitting the `: label`, so none collides with the `:` separator.
            let (right, lollipop_to) = split_leading_lollipop(line[after_tok..].trim());
            let (right, to_card) = split_leading_card(&right);
            let (right, to_class) = extract_inline_class(right.trim());
            let (to_clean, label) = match right.split_once(':') {
                Some((a, b)) => (a.trim().to_string(), Some(b.trim().to_string())),
                None => (right.trim().to_string(), None),
            };
            get_class(&mut diag, &mut by_name, &from);
            get_class(&mut diag, &mut by_name, &to_clean);
            if let Some(c) = from_class {
                let cls = get_class(&mut diag, &mut by_name, &from);
                if !cls.classes.contains(&c) {
                    cls.classes.push(c);
                }
            }
            if let Some(c) = to_class {
                let cls = get_class(&mut diag, &mut by_name, &to_clean);
                if !cls.classes.contains(&c) {
                    cls.classes.push(c);
                }
            }
            diag.relations.push(ClassRelation {
                from,
                to: to_clean,
                kind,
                label,
                from_card,
                to_card,
                reversed,
                to_kind,
                lollipop_from,
                lollipop_to,
            });
            continue;
        }

        // Standalone annotation on its own line, either order:
        //   `Shape <<interface>>`   or   `<<interface>> Shape`
        if let Some((cls_name, stereo)) = parse_standalone_annotation(line) {
            let cls = get_class(&mut diag, &mut by_name, &cls_name);
            cls.stereotype = Some(stereo);
            continue;
        }

        return Err(ParseError::unknown(
            line_no,
            format!("unrecognized class statement: '{line}'"),
        ));
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}
