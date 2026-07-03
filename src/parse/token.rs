//! Quote-aware tokenizing primitives shared by the per-diagram parsers.
//!
//! Mermaid labels, cardinalities, CSV fields, and attribute blocks embed
//! delimiters (commas, colons, relation glyphs) inside `"…"` runs, so a naive
//! `split`/`find` would break them apart. These helpers scan while respecting
//! quoted regions. Each parser used to carry its own near-identical copy.

/// Strip one surrounding pair of double quotes after trimming, e.g.
/// `  "foo"  ` → `foo`. Unquoted input is returned trimmed but unchanged.
pub(crate) fn unquote(s: &str) -> &str {
    unquote_with(s, &['"'])
}

/// Like [`unquote`] but also honors single quotes (`'…'`). The stripped pair
/// must match (both `"` or both `'`).
pub(crate) fn unquote_any(s: &str) -> &str {
    unquote_with(s, &['"', '\''])
}

fn unquote_with<'a>(s: &'a str, quotes: &[char]) -> &'a str {
    let s = s.trim();
    let mut chars = s.chars();
    if let (Some(first), Some(last)) = (chars.next(), chars.next_back()) {
        if first == last && quotes.contains(&first) {
            return &s[first.len_utf8()..s.len() - last.len_utf8()];
        }
    }
    s
}

/// First byte offset of `needle` in `haystack` that lies outside any `"…"`
/// quoted region. Cardinalities like `"1..*"` embed relation tokens (`..`), so
/// token scanning must skip quoted text.
pub(crate) fn find_unquoted(haystack: &str, needle: &str) -> Option<usize> {
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

/// Split `s` on `delim` occurrences that sit outside any `"`/`'` quoted run.
/// Quote characters are retained in the parts, each part is trimmed, and empty
/// parts are dropped.
pub(crate) fn split_unquoted(s: &str, delim: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
                cur.push(c);
            }
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            None if c == delim => {
                if !cur.trim().is_empty() {
                    parts.push(cur.trim().to_string());
                }
                cur.clear();
            }
            None => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquote_strips_double_quotes() {
        assert_eq!(unquote("  \"hi\" "), "hi");
        assert_eq!(unquote("plain"), "plain");
        assert_eq!(unquote("'x'"), "'x'"); // single quotes untouched
        assert_eq!(unquote("\""), "\""); // lone quote is not a pair
    }

    #[test]
    fn unquote_any_strips_either() {
        assert_eq!(unquote_any("'hi'"), "hi");
        assert_eq!(unquote_any("\"hi\""), "hi");
        assert_eq!(unquote_any("\"mixed'"), "\"mixed'");
    }

    #[test]
    fn find_unquoted_skips_quoted_run() {
        assert_eq!(find_unquoted("a\"..\"..b", ".."), Some(5));
        assert_eq!(find_unquoted("\"..\"", ".."), None);
    }

    #[test]
    fn split_unquoted_respects_quotes() {
        assert_eq!(
            split_unquoted("a, \"b,c\", d", ','),
            vec!["a".to_string(), "\"b,c\"".to_string(), "d".to_string()]
        );
        assert!(split_unquoted("  ,  ,", ',').is_empty());
    }
}
