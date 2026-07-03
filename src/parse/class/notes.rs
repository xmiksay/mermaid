//! Class diagram notes, standalone annotations, and interactivity
//! (`click`/`link`/`callback`) parsing.

use crate::parse::{ClassNote, ClickAction};

/// Parse a `note` statement body (text after `note `): `"text"` (free) or
/// `for <Class> "text"` (attached). Surrounding quotes on the text are stripped.
pub(super) fn parse_note(rest: &str) -> ClassNote {
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
pub(super) fn strip_any_prefix<'a>(line: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| line.strip_prefix(p))
}

/// Parse the body of a `click`/`link`/`callback` statement (text after the
/// keyword) into `(class name, action)`. Modeled on the flowchart `click`
/// support:
///   `Shape "url" "tooltip"`         → hyperlink
///   `Shape href "url" "tooltip"`    → hyperlink
///   `Shape call fn() "tooltip"`     → callback
///   `Shape callbackFn "tooltip"`    → callback
pub(super) fn parse_interaction(rest: &str) -> Option<(String, ClickAction)> {
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
pub(super) fn parse_standalone_annotation(line: &str) -> Option<(String, String)> {
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

#[cfg(test)]
mod tests {
    use super::super::parse;
    use crate::parse::{ClassDiagram, ClickAction, UmlClass};

    fn class<'a>(d: &'a ClassDiagram, name: &str) -> &'a UmlClass {
        d.classes.iter().find(|c| c.name == name).unwrap()
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
}
