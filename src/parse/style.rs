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
}
