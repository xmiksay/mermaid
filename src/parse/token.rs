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

/// Remove a `:::class` token from `raw`, returning the remaining text (with the
/// token excised, so a trailing `: label` survives) and the class name. Only the
/// first occurrence is handled.
pub(crate) fn extract_inline_class(raw: &str) -> (String, Option<String>) {
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

/// Split an `id[Label]` form into `(id, label)`; a plain string reuses itself as
/// both id and label. A bracket form with an empty prefix (`[Label]`) reuses the
/// label as the id. A surrounding pair of quotes is stripped from each side.
pub(crate) fn split_id_label(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(open) = s.find('[') {
        if s.ends_with(']') {
            let id = s[..open].trim();
            let label = s[open + 1..s.len() - 1].trim();
            let id = if id.is_empty() { label } else { id };
            return (unquote(id).to_string(), unquote(label).to_string());
        }
    }
    let s = unquote(s);
    (s.to_string(), s.to_string())
}

/// Parse an `@{ key: value, … }` attribute-block body (with the surrounding
/// `@{`/`}` already stripped) into trimmed, unquoted `(key, value)` pairs.
/// Commas separate pairs and the first `:` splits each; both are honored only
/// outside quotes so a quoted value may embed either. Fragments without a `:`
/// are skipped.
pub(crate) fn parse_attr_pairs(body: &str) -> Vec<(String, String)> {
    split_unquoted(body, ',')
        .into_iter()
        .filter_map(|part| {
            part.split_once(':').map(|(k, v)| {
                (
                    unquote_any(k.trim()).to_string(),
                    unquote_any(v.trim()).to_string(),
                )
            })
        })
        .collect()
}

/// Split `s` on top-level `delim` occurrences — those at bracket depth zero and
/// outside any `"…"` quoted run. `(`/`[`/`{` count depth up and their mates
/// count it down; a quote suppresses both bracket counting and delimiter
/// matching so a bracket or delimiter inside `"…"` stays literal. Parts are
/// returned verbatim (untrimmed, quotes retained); the trailing part is always
/// included, so empty input yields one empty string.
pub(crate) fn split_top_level(s: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    for c in s.chars() {
        if c == '"' {
            in_quote = !in_quote;
            cur.push(c);
            continue;
        }
        if in_quote {
            cur.push(c);
            continue;
        }
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                cur.push(c);
            }
            d if d == delim && depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
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

    #[test]
    fn extract_inline_class_excises_token() {
        assert_eq!(
            extract_inline_class("Dog:::bar : owns"),
            ("Dog : owns".to_string(), Some("bar".to_string()))
        );
        assert_eq!(extract_inline_class("plain"), ("plain".to_string(), None));
    }

    #[test]
    fn split_id_label_forms() {
        assert_eq!(
            split_id_label("id[Task 1]"),
            ("id".to_string(), "Task 1".to_string())
        );
        assert_eq!(
            split_id_label("[Only Label]"),
            ("Only Label".to_string(), "Only Label".to_string())
        );
        assert_eq!(
            split_id_label("bare"),
            ("bare".to_string(), "bare".to_string())
        );
        // Surrounding quotes are stripped from both id and label.
        assert_eq!(
            split_id_label("\"quoted\""),
            ("quoted".to_string(), "quoted".to_string())
        );
    }

    #[test]
    fn parse_attr_pairs_unquotes_both_sides() {
        assert_eq!(
            parse_attr_pairs(" assigned: 'Alice', priority: \"High\" "),
            vec![
                ("assigned".to_string(), "Alice".to_string()),
                ("priority".to_string(), "High".to_string()),
            ]
        );
        // A quoted value may embed the comma/colon delimiters.
        assert_eq!(
            parse_attr_pairs("label: \"a, b: c\""),
            vec![("label".to_string(), "a, b: c".to_string())]
        );
        // Keyless fragments are dropped.
        assert!(parse_attr_pairs("nonsense").is_empty());
    }

    #[test]
    fn split_top_level_respects_depth_and_quotes() {
        assert_eq!(
            split_top_level("a[\"x,y\"], b{1, 2}, c", ','),
            vec![
                "a[\"x,y\"]".to_string(),
                " b{1, 2}".to_string(),
                " c".to_string()
            ]
        );
        // A bracket inside quotes does not shift the depth.
        assert_eq!(
            split_top_level("\"a[b\", c", ','),
            vec!["\"a[b\"".to_string(), " c".to_string()]
        );
        // The trailing part is always present, even when empty.
        assert_eq!(
            split_top_level("a,", ','),
            vec!["a".to_string(), String::new()]
        );
    }
}
