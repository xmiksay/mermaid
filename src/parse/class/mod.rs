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

use super::ast::{
    ClassDiagram, ClassMember, ClassRelation, FlowDirection, MemberKind, Namespace, Style,
    UmlClass, Visibility,
};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

mod notes;
mod relation;

use notes::{parse_interaction, parse_note, parse_standalone_annotation, strip_any_prefix};
use relation::{
    find_relation, is_reversed_token, split_leading_card, split_leading_lollipop,
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
                return Err(ParseError::Syntax {
                    message: "expected 'classDiagram' header".into(),
                    line: line_no,
                });
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
                    return Err(ParseError::Syntax {
                        message: format!("unknown direction: '{other}'"),
                        line: line_no,
                    })
                }
            };
            continue;
        }

        if let Some(rest) = line.strip_prefix("namespace ") {
            let inner = rest.trim().trim_end_matches('{').trim();
            namespace_stack.push(inner.to_string());
            diag.namespaces.push(Namespace {
                name: inner.to_string(),
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
            if let Some(stereo) = take_stereotype(line) {
                let cls = get_class(&mut diag, &mut by_name, class_name);
                cls.stereotype = Some(stereo);
            } else {
                let member = parse_member(line);
                let cls = get_class(&mut diag, &mut by_name, class_name);
                cls.members.push(member);
            }
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
            if let Some(ns_name) = namespace_stack.last() {
                if let Some(ns) = diag.namespaces.iter_mut().find(|n| n.name == *ns_name) {
                    if !ns.class_names.contains(&added_name) {
                        ns.class_names.push(added_name);
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
        if let Some(rest) = strip_any_prefix(line, &["click ", "link ", "callback "]) {
            if let Some((name, action)) = parse_interaction(rest) {
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
            // Left end: `Class[:::style] ["card"] [()]`. The lollipop `()` sits
            // right against the token, so strip it before the multiplicity.
            let (left, lollipop_from) = split_trailing_lollipop(line[..tok_pos].trim());
            let (left, from_class) = extract_inline_class(&left);
            let (from, from_card) = split_trailing_card(&left);
            // Right end: `[()] ["card"] Class[:::style] [: label]`. Strip the
            // lollipop, then the leading multiplicity and any `:::style` before
            // splitting the `: label`, so none collides with the `:` separator.
            let (right, lollipop_to) = split_leading_lollipop(line[tok_pos + tok.len()..].trim());
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
                reversed: is_reversed_token(tok),
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

        return Err(ParseError::Syntax {
            message: format!("unrecognized class statement: '{line}'"),
            line: line_no,
        });
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

fn handle_class_decl(
    rest: &str,
    diag: &mut ClassDiagram,
    by_name: &mut HashMap<String, usize>,
    in_block: &mut Option<String>,
) -> String {
    let rest = rest.trim();
    let (name_part, after_brace) = match rest.split_once('{') {
        Some((a, b)) => (a.trim(), Some(b)),
        None => (rest, None),
    };
    let (name_part, inline_class) = extract_inline_class(name_part);
    let (name_part, label) = extract_class_label(&name_part);
    let (name, stereo) = if let Some(i) = name_part.find("<<") {
        let n = name_part[..i].trim();
        let s = name_part[i + 2..].trim_end_matches(">>").trim();
        (n, Some(s.to_string()))
    } else {
        (name_part.as_str(), None)
    };
    let cls = get_class(diag, by_name, name);
    if let Some(s) = stereo {
        cls.stereotype = Some(s);
    }
    if let Some(l) = label {
        cls.label = Some(l);
    }
    if let Some(c) = inline_class {
        if !cls.classes.contains(&c) {
            cls.classes.push(c);
        }
    }
    if after_brace.is_some() {
        *in_block = Some(name.to_string());
    }
    name.to_string()
}

/// `classDef <name>[,<name2>] <props>` — define style classes.
fn handle_class_def(rest: &str, diag: &mut ClassDiagram) {
    let Some((names, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let style = parse_style_props(props);
    for name in names.split(',') {
        let name = name.trim();
        if !name.is_empty() {
            diag.class_defs.insert(name.to_string(), style.clone());
        }
    }
}

/// `style <ClassName> <props>` — inline style on a single class.
fn handle_style(rest: &str, diag: &mut ClassDiagram, by_name: &mut HashMap<String, usize>) {
    let Some((name, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let cls = get_class(diag, by_name, name.trim());
    cls.style = parse_style_props(props);
}

/// `cssClass "id1,id2" <className>` — apply a style class to classes.
fn handle_css_class(rest: &str, diag: &mut ClassDiagram, by_name: &mut HashMap<String, usize>) {
    let Some((quoted, class_name)) = rest.trim().rsplit_once(char::is_whitespace) else {
        return;
    };
    let class_name = class_name.trim();
    if class_name.is_empty() {
        return;
    }
    for id in quoted.trim().trim_matches('"').split(',') {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        let cls = get_class(diag, by_name, id);
        if !cls.classes.iter().any(|c| c == class_name) {
            cls.classes.push(class_name.to_string());
        }
    }
}

/// Remove a `:::class` token from `raw`, returning the remaining text and the
/// class name (first occurrence only).
fn extract_inline_class(raw: &str) -> (String, Option<String>) {
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

/// Split a `Name["display label"]` declaration into the bare name and the
/// optional label, unquoting the bracket content. Generics use `~T~`, so a `[`
/// unambiguously opens a label here.
fn extract_class_label(raw: &str) -> (String, Option<String>) {
    let raw = raw.trim();
    if let (Some(open), Some(close)) = (raw.find('['), raw.rfind(']')) {
        if close > open {
            let label = raw[open + 1..close].trim().trim_matches('"').trim();
            let rest = format!("{}{}", &raw[..open], &raw[close + 1..]);
            let label = (!label.is_empty()).then(|| label.to_string());
            return (rest.trim().to_string(), label);
        }
    }
    (raw.to_string(), None)
}

fn take_stereotype(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("<<") {
        let s = rest.trim_end_matches(">>").trim();
        return Some(s.to_string());
    }
    None
}

fn parse_member(s: &str) -> ClassMember {
    let s = s.trim();
    let (vis, rest) = match s.chars().next() {
        Some('+') => (Visibility::Public, &s[1..]),
        Some('-') => (Visibility::Private, &s[1..]),
        Some('#') => (Visibility::Protected, &s[1..]),
        Some('~') => (Visibility::Package, &s[1..]),
        _ => (Visibility::Default, s),
    };
    let rest = rest.trim().to_string();
    let kind = if rest.contains('(') {
        MemberKind::Method
    } else {
        MemberKind::Attribute
    };
    ClassMember {
        visibility: vis,
        text: rest,
        kind,
    }
}

fn get_class<'a>(
    diag: &'a mut ClassDiagram,
    by_name: &mut HashMap<String, usize>,
    name: &str,
) -> &'a mut UmlClass {
    if !by_name.contains_key(name) {
        by_name.insert(name.to_string(), diag.classes.len());
        diag.classes.push(UmlClass {
            name: name.to_string(),
            label: None,
            stereotype: None,
            members: Vec::new(),
            classes: Vec::new(),
            style: Style::new(),
            click: None,
        });
    }
    let i = by_name[name];
    &mut diag.classes[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_header_alias() {
        // `classDiagram-v2` is an upstream alias for `classDiagram`.
        let d = parse("classDiagram-v2\nAnimal <|-- Dog\n").unwrap();
        assert_eq!(d.relations.len(), 1);
        // The dispatcher accepts the alias too.
        assert!(matches!(
            crate::parse("classDiagram-v2\nAnimal <|-- Dog\n").unwrap(),
            crate::Diagram::Class(_)
        ));
    }

    #[test]
    fn block_class_members() {
        let s = "classDiagram\n\
                 class Animal {\n\
                 +String name\n\
                 +int age\n\
                 +sleep()\n\
                 }\n";
        let d = parse(s).unwrap();
        assert_eq!(d.classes.len(), 1);
        let a = &d.classes[0];
        assert_eq!(a.name, "Animal");
        assert_eq!(a.members.len(), 3);
        assert_eq!(a.members[0].visibility, Visibility::Public);
        assert_eq!(a.members[0].kind, MemberKind::Attribute);
        assert_eq!(a.members[2].kind, MemberKind::Method);
    }

    #[test]
    fn shorthand_members() {
        let s = "classDiagram\n\
                 Animal : +String name\n\
                 Animal : -age int\n\
                 Animal : +sleep()\n";
        let d = parse(s).unwrap();
        assert_eq!(d.classes[0].members.len(), 3);
    }

    #[test]
    fn stereotype_recognized() {
        let s = "classDiagram\nclass Logger {\n<<interface>>\n+log()\n}\n";
        let d = parse(s).unwrap();
        assert_eq!(d.classes[0].stereotype.as_deref(), Some("interface"));
    }

    fn class<'a>(d: &'a ClassDiagram, name: &str) -> &'a UmlClass {
        d.classes.iter().find(|c| c.name == name).unwrap()
    }

    #[test]
    fn classdef_style_and_cssclass() {
        let s = "classDiagram\nAnimal --> Dog\nclassDef foo fill:#0f0\ncssClass \"Animal,Dog\" foo\nstyle Dog stroke:#333\n";
        let d = parse(s).unwrap();
        assert!(d.class_defs.contains_key("foo"));
        assert_eq!(class(&d, "Animal").classes, vec!["foo".to_string()]);
        assert_eq!(class(&d, "Dog").classes, vec!["foo".to_string()]);
        assert_eq!(
            class(&d, "Dog").style,
            vec![("stroke".to_string(), "#333".to_string())]
        );
    }

    #[test]
    fn class_label_sets_display_not_name() {
        let d = parse("classDiagram\nclass Animal[\"Animal with a label\"]\n").unwrap();
        // Exactly one class, named `Animal`, with the bracket text as its label.
        assert_eq!(d.classes.len(), 1);
        assert_eq!(d.classes[0].name, "Animal");
        assert_eq!(d.classes[0].label.as_deref(), Some("Animal with a label"));
    }

    #[test]
    fn class_label_with_body() {
        let d = parse("classDiagram\nclass Animal[\"A label\"] {\n+eat()\n}\n").unwrap();
        assert_eq!(d.classes.len(), 1);
        assert_eq!(d.classes[0].name, "Animal");
        assert_eq!(d.classes[0].label.as_deref(), Some("A label"));
        assert_eq!(d.classes[0].members.len(), 1);
    }

    #[test]
    fn inline_class_on_decl_and_relation() {
        let s = "classDiagram\nclass Animal:::foo\nAnimal --> Dog:::bar : owns\n";
        let d = parse(s).unwrap();
        assert_eq!(class(&d, "Animal").classes, vec!["foo".to_string()]);
        assert_eq!(class(&d, "Dog").classes, vec!["bar".to_string()]);
        assert_eq!(d.relations[0].label.as_deref(), Some("owns"));
    }
}
