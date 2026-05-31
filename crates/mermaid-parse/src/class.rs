//! Class diagram parser (v0.1 subset).
//!
//! Supports:
//!   * Header: `classDiagram`.
//!   * Class blocks: `class X { ... }` with member lines inside.
//!   * Member shorthand: `X : +method()`, `X : -attr int`.
//!   * Stereotype: `class X { <<interface>> ... }` or `X <<abstract>>`.
//!   * Relationships:
//!     `<|--` `<|..`  inheritance / realization
//!     `*--`         composition
//!     `o--`         aggregation
//!     `-->`         association (with arrow)
//!     `--`          link
//!     `..`          dashed link
//!     `..>`         dependency
//!     With optional `: label`.

use std::collections::HashMap;

use crate::ast::{
    ClassDiagram, ClassMember, ClassRelation, ClassRelationKind, MemberKind, UmlClass, Visibility,
};
use crate::{strip_comment, ParseError};

const RELATIONS: &[(&str, ClassRelationKind)] = &[
    ("<|..", ClassRelationKind::Realization),
    ("..|>", ClassRelationKind::Realization),
    ("<|--", ClassRelationKind::Inheritance),
    ("--|>", ClassRelationKind::Inheritance),
    ("*--", ClassRelationKind::Composition),
    ("--*", ClassRelationKind::Composition),
    ("o--", ClassRelationKind::Aggregation),
    ("--o", ClassRelationKind::Aggregation),
    ("..>", ClassRelationKind::Dependency),
    ("<..", ClassRelationKind::Dependency),
    ("-->", ClassRelationKind::Association),
    ("<--", ClassRelationKind::Association),
    ("--", ClassRelationKind::Link),
    ("..", ClassRelationKind::LinkDashed),
];

pub(crate) fn parse(input: &str) -> Result<ClassDiagram, ParseError> {
    let mut diag = ClassDiagram::default();
    let mut header_seen = false;
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut in_block: Option<String> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "classDiagram" {
                return Err(ParseError::Syntax {
                    message: "expected 'classDiagram' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
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

        if let Some(rest) = line.strip_prefix("class ") {
            handle_class_decl(rest, &mut diag, &mut by_name, &mut in_block);
            continue;
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
            let from = line[..tok_pos].trim().to_string();
            let after = &line[tok_pos + tok.len()..];
            let (to_part, label) = match after.split_once(':') {
                Some((a, b)) => (a.trim().to_string(), Some(b.trim().to_string())),
                None => (after.trim().to_string(), None),
            };
            // Strip role multiplicities like "1..n" surrounded by quotes — keep label only.
            let to_clean = to_part.trim().trim_matches('"').to_string();
            get_class(&mut diag, &mut by_name, &from);
            get_class(&mut diag, &mut by_name, &to_clean);
            diag.relations.push(ClassRelation {
                from,
                to: to_clean,
                kind: normalize_direction(tok, kind),
                label,
            });
            continue;
        }

        // Stereotype on its own line: `ClassName <<interface>>`
        if let Some(idx) = line.find("<<") {
            let cls_name = line[..idx].trim();
            let stereo = line[idx + 2..].trim_end_matches(">>").trim();
            let cls = get_class(&mut diag, &mut by_name, cls_name);
            cls.stereotype = Some(stereo.to_string());
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
) {
    let rest = rest.trim();
    let (name_part, after_brace) = match rest.split_once('{') {
        Some((a, b)) => (a.trim(), Some(b)),
        None => (rest, None),
    };
    let (name, stereo) = if let Some(i) = name_part.find("<<") {
        let n = name_part[..i].trim();
        let s = name_part[i + 2..].trim_end_matches(">>").trim();
        (n, Some(s.to_string()))
    } else {
        (name_part, None)
    };
    let cls = get_class(diag, by_name, name);
    if let Some(s) = stereo {
        cls.stereotype = Some(s);
    }
    if let Some(_remaining_after_brace) = after_brace {
        *in_block = Some(name.to_string());
    }
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

fn find_relation(line: &str) -> Option<(usize, &'static str, ClassRelationKind)> {
    let mut best: Option<(usize, &'static str, ClassRelationKind)> = None;
    for (tok, kind) in RELATIONS {
        if let Some(pos) = line.find(tok) {
            let candidate = (pos, *tok, *kind);
            best = match best {
                Some((bp, bt, _)) if bp < pos => Some((bp, bt, best.unwrap().2)),
                Some((bp, bt, _)) if bp == pos && bt.len() > tok.len() => {
                    Some((bp, bt, best.unwrap().2))
                }
                _ => Some(candidate),
            };
        }
    }
    best
}

fn normalize_direction(tok: &str, kind: ClassRelationKind) -> ClassRelationKind {
    // Reverse-direction tokens like `<|..`, `--|>`, `--*`, `--o`, `<--`, `<..`
    // are mirrored. For UML we don't track direction in the AST beyond the kind
    // — the from/to ordering already encodes it.
    let _ = tok;
    kind
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
            stereotype: None,
            members: Vec::new(),
        });
    }
    let i = by_name[name];
    &mut diag.classes[i]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn relations() {
        let s = "classDiagram\nAnimal <|-- Dog\nCar *-- Wheel\nUser ..|> Service\n";
        let d = parse(s).unwrap();
        assert_eq!(d.relations.len(), 3);
        assert_eq!(d.relations[0].kind, ClassRelationKind::Inheritance);
        assert_eq!(d.relations[1].kind, ClassRelationKind::Composition);
        assert_eq!(d.relations[2].kind, ClassRelationKind::Realization);
    }

    #[test]
    fn stereotype_recognized() {
        let s = "classDiagram\nclass Logger {\n<<interface>>\n+log()\n}\n";
        let d = parse(s).unwrap();
        assert_eq!(d.classes[0].stereotype.as_deref(), Some("interface"));
    }

    #[test]
    fn relation_with_label() {
        let s = "classDiagram\nCar --> Engine : has\n";
        let d = parse(s).unwrap();
        assert_eq!(d.relations[0].label.as_deref(), Some("has"));
    }
}
