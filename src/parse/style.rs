//! Shared parsing for Mermaid style directives (`style`, `classDef`,
//! `linkStyle`). The body of each directive is a comma-separated list of
//! `key:value` CSS-ish declarations.

use super::ast::Style;

/// Parse `fill:#f9f,stroke:#333,stroke-width:4px` into ordered key/value pairs.
///
/// A `\,` escape is a literal comma inside a value (e.g.
/// `stroke-dasharray:5\,5`). Fragments without a `:` or that are empty are
/// skipped. Keys and values are trimmed.
pub(crate) fn parse_style_props(s: &str) -> Style {
    let mut out = Style::new();
    for frag in split_escaped_commas(s) {
        let frag = frag.trim();
        if frag.is_empty() {
            continue;
        }
        if let Some((k, v)) = frag.split_once(':') {
            out.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

/// Split a `<id-list> <payload>` directive body (`classDef`/`class`/`style`)
/// into the comma-separated, trimmed, non-empty ids and the payload substring.
/// `payload_is_last` selects the split point: `false` (`classDef`/`style`)
/// treats everything after the first whitespace as the payload; `true` (`class`)
/// treats only the final whitespace-delimited token as the payload, leaving the
/// id-list in front. Returns `None` when the body lacks a whitespace separator
/// or has no non-empty ids, so the caller can raise its own error.
pub(crate) fn parse_multi_id_stmt(
    body: &str,
    payload_is_last: bool,
) -> Option<(Vec<String>, &str)> {
    let body = body.trim();
    let (ids, payload) = if payload_is_last {
        body.rsplit_once(char::is_whitespace)?
    } else {
        body.split_once(char::is_whitespace)?
    };
    let ids: Vec<String> = ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    (!ids.is_empty()).then_some((ids, payload))
}

fn split_escaped_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&',') {
            cur.push(',');
            chars.next();
        } else if c == ',' {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    parts.push(cur);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_props() {
        let s = parse_style_props("fill:#f9f,stroke:#333,stroke-width:4px");
        assert_eq!(
            s,
            vec![
                ("fill".to_string(), "#f9f".to_string()),
                ("stroke".to_string(), "#333".to_string()),
                ("stroke-width".to_string(), "4px".to_string()),
            ]
        );
    }

    #[test]
    fn trims_whitespace() {
        let s = parse_style_props("  fill : #fff ,  color:#000 ");
        assert_eq!(s[0], ("fill".to_string(), "#fff".to_string()));
        assert_eq!(s[1], ("color".to_string(), "#000".to_string()));
    }

    #[test]
    fn honours_escaped_comma() {
        let s = parse_style_props("stroke-dasharray:5\\,5");
        assert_eq!(s, vec![("stroke-dasharray".to_string(), "5,5".to_string())]);
    }

    #[test]
    fn skips_empty_and_keyless() {
        let s = parse_style_props("fill:#f9f,,nonsense,stroke:#000");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].0, "fill");
        assert_eq!(s[1].0, "stroke");
    }

    #[test]
    fn multi_id_stmt_splits_ids_and_payload() {
        // classDef/style: payload is everything after the first whitespace; the
        // id-list is comma-separated with no interior spaces (upstream grammar).
        let (ids, payload) = parse_multi_id_stmt("A,B,C fill:#f9f,stroke:#000", false).unwrap();
        assert_eq!(ids, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(payload, "fill:#f9f,stroke:#000");

        // class: payload is the trailing token, id-list in front.
        let (ids, payload) = parse_multi_id_stmt("A,B foo", true).unwrap();
        assert_eq!(ids, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(payload, "foo");

        // Empty ids and missing separators return None.
        assert!(parse_multi_id_stmt("A", false).is_none());
        assert!(parse_multi_id_stmt(" , foo", true).is_none());
    }
}
