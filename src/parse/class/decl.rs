//! Class declaration, member, and style-statement parsing: `class X { … }`
//! bodies, member rows, `classDef`/`style`/`cssClass`, and the shared
//! `get_class` interner.

use std::collections::HashMap;

use crate::parse::style::{parse_multi_id_stmt, parse_style_props};
use crate::parse::token::extract_inline_class;
use crate::parse::{ClassDiagram, ClassMember, MemberKind, Style, UmlClass, Visibility};

pub(super) fn handle_class_decl(
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
    if let Some(after) = after_brace {
        match after.find('}') {
            // One-line body `class Duck { +swim() }`: the block opens and closes
            // on the same line, so parse the inline members and keep it closed —
            // otherwise the block swallows every following statement.
            Some(close) => {
                let body = after[..close].trim();
                if !body.is_empty() {
                    add_member_line(diag, by_name, name, body);
                }
            }
            None => *in_block = Some(name.to_string()),
        }
    }
    name.to_string()
}

/// Add one member line to a class — either a `<<stereotype>>` or a member row.
/// Shared by the multi-line block body and the one-line `{ … }` body.
pub(super) fn add_member_line(
    diag: &mut ClassDiagram,
    by_name: &mut HashMap<String, usize>,
    class_name: &str,
    line: &str,
) {
    if let Some(stereo) = take_stereotype(line) {
        get_class(diag, by_name, class_name).stereotype = Some(stereo);
    } else {
        let member = parse_member(line);
        get_class(diag, by_name, class_name).members.push(member);
    }
}

/// `classDef <name>[,<name2>] <props>` — define style classes.
pub(super) fn handle_class_def(rest: &str, diag: &mut ClassDiagram) {
    let Some((names, props)) = parse_multi_id_stmt(rest, false) else {
        return;
    };
    let style = parse_style_props(props);
    for name in names {
        diag.class_defs.insert(name, style.clone());
    }
}

/// `style <ClassName> <props>` — inline style on a single class.
pub(super) fn handle_style(
    rest: &str,
    diag: &mut ClassDiagram,
    by_name: &mut HashMap<String, usize>,
) {
    let Some((name, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let cls = get_class(diag, by_name, name.trim());
    cls.style = parse_style_props(props);
}

/// `cssClass "id1,id2" <className>` — apply a style class to classes.
pub(super) fn handle_css_class(
    rest: &str,
    diag: &mut ClassDiagram,
    by_name: &mut HashMap<String, usize>,
) {
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

/// Split a `Name["display label"]` declaration into the bare name and the
/// optional label, unquoting the bracket content. Generics use `~T~`, so a `[`
/// unambiguously opens a label here.
pub(super) fn extract_class_label(raw: &str) -> (String, Option<String>) {
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

pub(super) fn parse_member(s: &str) -> ClassMember {
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

pub(super) fn get_class<'a>(
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
