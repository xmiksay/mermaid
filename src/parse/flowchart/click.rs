//! Flowchart `click <id> …` directive parser.
//!
//! Binds a hyperlink (`ClickAction::Href`) or JS callback
//! (`ClickAction::Callback`) to a node, with optional tooltip and link target.

use super::super::ast::ClickAction;

/// Parse a `click <id> …` directive body (text after `click `) into the node
/// id and its bound action. Returns `None` if the line is malformed.
///
/// Recognized forms (tooltips and `_target` are optional throughout):
///   `click A "url" "tooltip" _blank`   → hyperlink
///   `click A href "url" "tooltip"`      → hyperlink
///   `click A callback "tooltip"`        → JS callback
///   `click A call callback() "tooltip"` → JS callback
pub(crate) fn parse_click(rest: &str) -> Option<(String, ClickAction)> {
    let toks = click_tokens(rest);
    let (id_tok, args) = toks.split_first()?;
    let id = id_tok.value.clone();
    let head = args.first()?;

    if !head.quoted && head.value == "href" {
        let url = args.get(1)?.value.clone();
        let (tooltip, target) = tooltip_and_target(&args[2..]);
        return Some((
            id,
            ClickAction::Href {
                url,
                tooltip,
                target,
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
        let (tooltip, target) = tooltip_and_target(&args[1..]);
        return Some((
            id,
            ClickAction::Href {
                url,
                tooltip,
                target,
            },
        ));
    }
    // Bare token → callback function name.
    let function = head.value.clone();
    let tooltip = args.get(1).map(|t| t.value.clone());
    Some((id, ClickAction::Callback { function, tooltip }))
}

struct ClickToken {
    quoted: bool,
    value: String,
}

/// Split a click-directive body into whitespace-delimited tokens, treating a
/// `"…"` run as a single (quoted) token so URLs and tooltips keep their spaces.
fn click_tokens(s: &str) -> Vec<ClickToken> {
    let bytes = s.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
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
            tokens.push(ClickToken {
                quoted: true,
                value: s[start..i].to_string(),
            });
            if i < bytes.len() {
                i += 1; // closing quote
            }
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
                i += 1;
            }
            tokens.push(ClickToken {
                quoted: false,
                value: s[start..i].to_string(),
            });
        }
    }
    tokens
}

/// From the trailing tokens of a hyperlink `click`, pick the first quoted token
/// as the tooltip and the first `_`-prefixed bare token (e.g. `_blank`) as the
/// link target.
fn tooltip_and_target(rest: &[ClickToken]) -> (Option<String>, Option<String>) {
    let mut tooltip = None;
    let mut target = None;
    for tok in rest {
        if tok.quoted {
            tooltip.get_or_insert_with(|| tok.value.clone());
        } else if tok.value.starts_with('_') {
            target.get_or_insert_with(|| tok.value.clone());
        }
    }
    (tooltip, target)
}

#[cfg(test)]
mod tests {
    use super::super::super::ast::{ClickAction, FlowNode, FlowchartDiagram};
    use super::super::parse;

    fn node<'a>(d: &'a FlowchartDiagram, id: &str) -> &'a FlowNode {
        d.nodes.iter().find(|n| n.id == id).unwrap()
    }

    #[test]
    fn click_href_with_tooltip() {
        let d =
            parse("flowchart TD\nA-->B\nclick A \"https://example.com\" \"tooltip\"\n").unwrap();
        assert_eq!(d.edges.len(), 1);
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Href {
                url: "https://example.com".into(),
                tooltip: Some("tooltip".into()),
                target: None,
            })
        );
    }

    #[test]
    fn click_href_keyword_and_target() {
        let d = parse("flowchart TD\nA-->B\nclick A href \"http://x\" \"tip\" _blank\n").unwrap();
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Href {
                url: "http://x".into(),
                tooltip: Some("tip".into()),
                target: Some("_blank".into()),
            })
        );
    }

    #[test]
    fn click_callback_bare() {
        let d = parse("flowchart TD\nA-->B\nclick A callback \"a tip\"\n").unwrap();
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Callback {
                function: "callback".into(),
                tooltip: Some("a tip".into()),
            })
        );
    }

    #[test]
    fn click_callback_call_keyword() {
        let d = parse("flowchart TD\nA-->B\nclick A call handler()\n").unwrap();
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Callback {
                function: "handler()".into(),
                tooltip: None,
            })
        );
    }

    #[test]
    fn click_before_node_declared_creates_it() {
        let d = parse("flowchart TD\nclick Z \"http://z\"\nZ-->B\n").unwrap();
        assert!(node(&d, "Z").click.is_some());
        assert_eq!(d.edges.len(), 1);
    }
}
