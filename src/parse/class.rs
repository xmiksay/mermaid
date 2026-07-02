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
//!     With optional role multiplicities (`A "1" --> "*" B`) and `: label`.

use std::collections::HashMap;

use super::ast::{
    ClassDiagram, ClassMember, ClassNote, ClassRelation, ClassRelationKind, ClickAction,
    FlowDirection, MemberKind, Namespace, Style, UmlClass, Visibility,
};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

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
    let mut namespace_stack: Vec<String> = Vec::new();

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
            // Left end: `Class[:::style] ["card"]`.
            let (left, from_class) = extract_inline_class(line[..tok_pos].trim());
            let (from, from_card) = split_trailing_card(&left);
            // Right end: `["card"] Class[:::style] [: label]`. Strip the leading
            // multiplicity and any `:::style` before splitting the `: label`,
            // so neither collides with the `:` separator.
            let (right, to_card) = split_leading_card(line[tok_pos + tok.len()..].trim());
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

/// Parse a `note` statement body (text after `note `): `"text"` (free) or
/// `for <Class> "text"` (attached). Surrounding quotes on the text are stripped.
fn parse_note(rest: &str) -> ClassNote {
    let rest = rest.trim();
    if let Some(after) = rest.strip_prefix("for ") {
        let after = after.trim();
        // `<Class> "text"` — split on the opening quote if present, else on
        // the first whitespace run.
        if let Some(q) = after.find('"') {
            let target = after[..q].trim();
            let text = after[q..].trim().trim_matches('"');
            return ClassNote {
                target: (!target.is_empty()).then(|| target.to_string()),
                text: text.to_string(),
            };
        }
        let (target, text) = after.split_once(char::is_whitespace).unwrap_or((after, ""));
        return ClassNote {
            target: Some(target.trim().to_string()),
            text: text.trim().trim_matches('"').to_string(),
        };
    }
    ClassNote {
        target: None,
        text: rest.trim_matches('"').to_string(),
    }
}

/// Strip the first matching prefix from `line`, returning the remainder.
fn strip_any_prefix<'a>(line: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| line.strip_prefix(p))
}

/// Parse the body of a `click`/`link`/`callback` statement (text after the
/// keyword) into `(class name, action)`. Modeled on the flowchart `click`
/// support:
///   `Shape "url" "tooltip"`         → hyperlink
///   `Shape href "url" "tooltip"`    → hyperlink
///   `Shape call fn() "tooltip"`     → callback
///   `Shape callbackFn "tooltip"`    → callback
fn parse_interaction(rest: &str) -> Option<(String, ClickAction)> {
    let toks = quote_tokens(rest);
    let (id_tok, args) = toks.split_first()?;
    let id = id_tok.value.clone();
    let head = args.first()?;

    if !head.quoted && head.value == "href" {
        let url = args.get(1)?.value.clone();
        let tooltip = args.get(2).map(|t| t.value.clone());
        return Some((
            id,
            ClickAction::Href {
                url,
                tooltip,
                target: None,
            },
        ));
    }
    if !head.quoted && head.value == "call" {
        let function = args.get(1)?.value.clone();
        let tooltip = args.get(2).map(|t| t.value.clone());
        return Some((id, ClickAction::Callback { function, tooltip }));
    }
    if head.quoted {
        let url = head.value.clone();
        let tooltip = args.get(1).map(|t| t.value.clone());
        return Some((
            id,
            ClickAction::Href {
                url,
                tooltip,
                target: None,
            },
        ));
    }
    // Bare token → callback function name.
    let function = head.value.clone();
    let tooltip = args.get(1).map(|t| t.value.clone());
    Some((id, ClickAction::Callback { function, tooltip }))
}

struct QuoteToken {
    quoted: bool,
    value: String,
}

/// Split on whitespace, keeping a `"…"` run as one (quoted) token so URLs and
/// tooltips retain their spaces.
fn quote_tokens(s: &str) -> Vec<QuoteToken> {
    let bytes = s.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b'"' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            tokens.push(QuoteToken {
                quoted: true,
                value: s[start..i].to_string(),
            });
            if i < bytes.len() {
                i += 1; // closing quote
            }
        } else {
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            tokens.push(QuoteToken {
                quoted: false,
                value: s[start..i].to_string(),
            });
        }
    }
    tokens
}

/// Parse a standalone annotation line in either order — `Shape <<interface>>`
/// or `<<interface>> Shape` — into `(class name, stereotype)`. Requires a
/// balanced `<<…>>` and a non-empty class name on exactly one side.
fn parse_standalone_annotation(line: &str) -> Option<(String, String)> {
    let open = line.find("<<")?;
    let close = line[open + 2..].find(">>")? + open + 2;
    let stereo = line[open + 2..close].trim();
    let before = line[..open].trim();
    let after = line[close + 2..].trim();
    let name = match (before.is_empty(), after.is_empty()) {
        (false, true) => before,
        (true, false) => after,
        _ => return None,
    };
    if name.is_empty() || stereo.is_empty() {
        return None;
    }
    Some((name.to_string(), stereo.to_string()))
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

/// Find the first byte position of `needle` in `haystack` that lies outside of
/// any `"…"` quoted region. Cardinalities like `"1..*"` embed relation tokens
/// (`..`), so token scanning must ignore quoted text.
fn find_unquoted(haystack: &str, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let nb = needle.as_bytes();
    let mut in_quote = false;
    let mut i = 0;
    while i + nb.len() <= bytes.len() {
        if bytes[i] == b'"' {
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        if !in_quote && &bytes[i..i + nb.len()] == nb {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Strip a trailing `"card"` multiplicity, e.g. `Customer "1"` → (`Customer`, `1`).
fn split_trailing_card(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    if let Some(inner) = s.strip_suffix('"') {
        if let Some(open) = inner.rfind('"') {
            let card = inner[open + 1..].to_string();
            let rest = inner[..open].trim_end().to_string();
            return (rest, (!card.is_empty()).then_some(card));
        }
    }
    (s.to_string(), None)
}

/// Strip a leading `"card"` multiplicity, e.g. `"*" Order` → (`Order`, `*`).
fn split_leading_card(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"') {
        if let Some(close) = inner.find('"') {
            let card = inner[..close].to_string();
            let rest = inner[close + 1..].trim_start().to_string();
            return (rest, (!card.is_empty()).then_some(card));
        }
    }
    (s.to_string(), None)
}

fn find_relation(line: &str) -> Option<(usize, &'static str, ClassRelationKind)> {
    let mut best: Option<(usize, &'static str, ClassRelationKind)> = None;
    for (tok, kind) in RELATIONS {
        if let Some(pos) = find_unquoted(line, tok) {
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

/// A relation token is "reversed" when its decorated end (triangle/diamond/
/// circle/arrow) is on the left — attached to the `from` class — i.e. it opens
/// with `<`, `*`, or `o` (`<|--`, `<|..`, `*--`, `o--`, `<--`, `<..`). The
/// marker is then drawn at the `from` end instead of `to`. Plain links (`--`,
/// `..`) have no marker, so the flag is irrelevant for them.
fn is_reversed_token(tok: &str) -> bool {
    tok.starts_with(['<', '*', 'o'])
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
    fn reversed_tokens_flag_the_from_end() {
        let s = "classDiagram\n\
                 Animal <|-- Dog\n\
                 Dog --|> Animal\n\
                 A --* B\n\
                 A *-- B\n\
                 A --o B\n\
                 A <-- B\n\
                 A <.. B\n\
                 A -- B\n";
        let d = parse(s).unwrap();
        // from/to ordering (and thus layout) is preserved; only the marker end
        // moves. `<|--`/`*--`/`o--`/`<--`/`<..` are reversed (marker at `from`).
        assert!(d.relations[0].reversed); // Animal <|-- Dog
        assert_eq!(d.relations[0].from, "Animal");
        assert_eq!(d.relations[0].to, "Dog");
        assert!(!d.relations[1].reversed); // Dog --|> Animal
        assert!(!d.relations[2].reversed); // A --* B
        assert!(d.relations[3].reversed); // A *-- B
        assert!(!d.relations[4].reversed); // A --o B
        assert!(d.relations[5].reversed); // A <-- B
        assert!(d.relations[6].reversed); // A <.. B
        assert!(!d.relations[7].reversed); // A -- B (plain link)
    }

    #[test]
    fn cardinality_labels() {
        let d = parse("classDiagram\nCustomer \"1\" --> \"*\" Order\n").unwrap();
        assert_eq!(d.classes.len(), 2);
        assert_eq!(d.classes[0].name, "Customer");
        assert_eq!(d.classes[1].name, "Order");
        let r = &d.relations[0];
        assert_eq!(r.from, "Customer");
        assert_eq!(r.to, "Order");
        assert_eq!(r.from_card.as_deref(), Some("1"));
        assert_eq!(r.to_card.as_deref(), Some("*"));
        assert_eq!(r.kind, ClassRelationKind::Association);
    }

    #[test]
    fn cardinality_with_range_and_label() {
        let d = parse("classDiagram\nStudent \"1..*\" o-- \"0..1\" Course : enrolls\n").unwrap();
        let r = &d.relations[0];
        assert_eq!(r.from, "Student");
        assert_eq!(r.to, "Course");
        assert_eq!(r.from_card.as_deref(), Some("1..*"));
        assert_eq!(r.to_card.as_deref(), Some("0..1"));
        assert_eq!(r.label.as_deref(), Some("enrolls"));
        assert_eq!(r.kind, ClassRelationKind::Aggregation);
    }

    #[test]
    fn single_side_cardinality() {
        let d = parse("classDiagram\nA \"1\" --> B\nC --> \"*\" D\n").unwrap();
        assert_eq!(d.relations[0].from_card.as_deref(), Some("1"));
        assert_eq!(d.relations[0].to_card, None);
        assert_eq!(d.relations[0].to, "B");
        assert_eq!(d.relations[1].from_card, None);
        assert_eq!(d.relations[1].to_card.as_deref(), Some("*"));
        assert_eq!(d.relations[1].from, "C");
        assert_eq!(d.relations[1].to, "D");
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
    fn free_and_attached_notes() {
        let d = parse(
            "classDiagram\nclass Duck\nnote \"a general remark\"\nnote for Duck \"can fly\"\n",
        )
        .unwrap();
        assert_eq!(d.notes.len(), 2);
        assert_eq!(d.notes[0].target, None);
        assert_eq!(d.notes[0].text, "a general remark");
        assert_eq!(d.notes[1].target.as_deref(), Some("Duck"));
        assert_eq!(d.notes[1].text, "can fly");
        // The note-for class exists and no phantom class was created.
        assert_eq!(d.classes.len(), 1);
        assert_eq!(d.classes[0].name, "Duck");
    }

    #[test]
    fn standalone_annotation_both_orders() {
        // `<<interface>> Shape` (annotation-first) and `Shape2 <<service>>`
        // (name-first) both set the stereotype without a phantom empty class.
        let d = parse("classDiagram\n<<interface>> Shape\nShape2 <<service>>\n").unwrap();
        assert_eq!(d.classes.len(), 2);
        assert_eq!(class(&d, "Shape").stereotype.as_deref(), Some("interface"));
        assert_eq!(class(&d, "Shape2").stereotype.as_deref(), Some("service"));
        assert!(!d.classes.iter().any(|c| c.name.is_empty()));
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
    fn interactivity_lines_do_not_mangle() {
        let d = parse(
            "classDiagram\nclass Shape\nclick Shape href \"https://example.com\" \"tip\"\nlink Shape2 \"https://x.com\"\ncallback Shape3 handler \"a tip\"\n",
        )
        .unwrap();
        // No garbage classes named after the URL or member fragments.
        assert!(!d.classes.iter().any(|c| c.name.contains('/')));
        assert!(!d.classes.iter().any(|c| c.name.contains("link")));
        match class(&d, "Shape").click.as_ref().unwrap() {
            ClickAction::Href { url, tooltip, .. } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(tooltip.as_deref(), Some("tip"));
            }
            _ => panic!("expected href"),
        }
        match class(&d, "Shape2").click.as_ref().unwrap() {
            ClickAction::Href { url, .. } => assert_eq!(url, "https://x.com"),
            _ => panic!("expected href"),
        }
        match class(&d, "Shape3").click.as_ref().unwrap() {
            ClickAction::Callback { function, tooltip } => {
                assert_eq!(function, "handler");
                assert_eq!(tooltip.as_deref(), Some("a tip"));
            }
            _ => panic!("expected callback"),
        }
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
