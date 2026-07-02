//! Shared source *preamble* handling, run before per-diagram dispatch.
//!
//! Upstream Mermaid lets any diagram carry a chunk of cross-cutting metadata
//! that is not part of the diagram body:
//!
//! - **YAML frontmatter** delimited by `---` lines at the very top, carrying a
//!   `title:` and a nested `config:` block (we read `config.theme`).
//! - **`%%{init: {...}}%%` init directives** anywhere in the source; we read the
//!   `theme` key.
//! - **`accTitle:` / `accDescr:`** accessibility statements (the latter also in
//!   a `accDescr { … }` block form).
//!
//! [`strip`] pulls these out of the raw source, returning the extracted
//! [`DiagramMeta`] plus a cleaned source string with all of the above removed so
//! the line-oriented per-diagram scanners never see them.

use super::ast::DiagramMeta;

/// Extract the preamble metadata and return `(meta, cleaned_source)`.
pub fn strip(input: &str) -> (DiagramMeta, String) {
    let mut meta = DiagramMeta::default();
    let lines: Vec<&str> = input.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());

    let mut i = strip_frontmatter(&lines, &mut meta);

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if let Some(theme) = init_theme(trimmed) {
            // A `%%{init}%%` directive line: consume it (theme captured) and
            // keep nothing — the per-diagram scanner would otherwise see an
            // empty comment line, which is harmless, but dropping keeps the
            // cleaned source tidy.
            if meta.theme.is_none() {
                meta.theme = Some(theme);
            }
            i += 1;
            continue;
        }

        if let Some(rest) = strip_prefix_ci(trimmed, "accTitle:") {
            meta.acc_title = Some(rest.trim().to_string());
            i += 1;
            continue;
        }

        if let Some(rest) = strip_prefix_ci(trimmed, "accDescr:") {
            meta.acc_descr = Some(rest.trim().to_string());
            i += 1;
            continue;
        }

        // `accDescr {` … `}` multi-line block.
        if let Some(rest) = strip_prefix_ci(trimmed, "accDescr") {
            let rest = rest.trim_start();
            if rest.starts_with('{') {
                i = collect_descr_block(&lines, i, rest, &mut meta);
                continue;
            }
        }

        out.push(line);
        i += 1;
    }

    (meta, out.join("\n"))
}

/// Parse a leading `--- … ---` YAML frontmatter block, filling `title` and
/// `config.theme`. Returns the index of the first line *after* the block (or 0
/// if there is no frontmatter).
fn strip_frontmatter(lines: &[&str], meta: &mut DiagramMeta) -> usize {
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let Some(start) = first else { return 0 };
    if lines[start].trim() != "---" {
        return 0;
    }
    let Some(end) = lines[start + 1..].iter().position(|l| l.trim() == "---") else {
        return 0;
    };
    let end = start + 1 + end;

    let mut in_config = false;
    for line in &lines[start + 1..end] {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(v) = strip_prefix_ci(trimmed, "title:") {
            meta.title = Some(unquote(v.trim()).to_string());
            in_config = false;
        } else if trimmed.eq_ignore_ascii_case("config:") {
            in_config = true;
        } else if in_config && indented {
            if let Some(v) = strip_prefix_ci(trimmed, "theme:") {
                meta.theme = Some(unquote(v.trim()).to_string());
            }
        } else {
            in_config = false;
        }
    }

    // Skip the closing `---` and any blank lines directly after it.
    let mut next = end + 1;
    while next < lines.len() && lines[next].trim().is_empty() {
        next += 1;
    }
    next
}

/// Collect an `accDescr { … }` block starting at line `i` (whose content after
/// `accDescr` is `first`). Returns the index just past the closing `}`.
fn collect_descr_block(lines: &[&str], i: usize, first: &str, meta: &mut DiagramMeta) -> usize {
    let mut body = String::new();
    // Text may follow the opening brace on the same line.
    let after = first[1..].trim();
    if let Some(pos) = after.find('}') {
        meta.acc_descr = Some(after[..pos].trim().to_string());
        return i + 1;
    }
    if !after.is_empty() {
        body.push_str(after);
    }
    let mut j = i + 1;
    while j < lines.len() {
        let t = lines[j].trim();
        if let Some(pos) = t.find('}') {
            let head = t[..pos].trim();
            if !head.is_empty() {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(head);
            }
            j += 1;
            break;
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(t);
        j += 1;
    }
    meta.acc_descr = Some(body);
    j
}

/// If `line` is a `%%{init: … }%%` directive, return its `theme` value.
fn init_theme(line: &str) -> Option<String> {
    let inner = line.strip_prefix("%%{")?;
    let inner = inner.strip_suffix("%%").unwrap_or(inner);
    let inner = inner.strip_suffix("}").unwrap_or(inner);
    // Only treat it as an init directive (still consume the line regardless, so
    // callers drop it) — theme is optional.
    json_value(inner, "theme")
}

/// Pull a `"key": value` out of a loose JSON/`%%{…}` fragment. Handles quoted
/// and bare values; stops at `,`, `}` or whitespace.
fn json_value(s: &str, key: &str) -> Option<String> {
    let mut search = 0;
    while let Some(rel) = s[search..].find(key) {
        let at = search + rel;
        let before = s[..at].chars().last();
        let after_key = &s[at + key.len()..];
        // Require the match to be a bare or quoted key immediately before a `:`.
        let boundary_ok = before
            .map(|c| c == '"' || c == '\'' || c == '{' || c == ',' || c.is_whitespace())
            .unwrap_or(true);
        let after_trim = after_key.trim_start_matches(['"', '\'']).trim_start();
        if boundary_ok {
            if let Some(rest) = after_trim.strip_prefix(':') {
                let val = rest.trim_start().trim_start_matches(['"', '\'']);
                let end = val
                    .find(['"', '\'', ',', '}', ' ', '\t'])
                    .unwrap_or(val.len());
                let val = val[..end].trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
        search = at + key.len();
    }
    None
}

/// Case-insensitive `strip_prefix`.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// Strip a single pair of surrounding single or double quotes.
fn unquote(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_preamble_is_identity() {
        let (m, s) = strip("flowchart TD\nA --> B\n");
        assert_eq!(m, DiagramMeta::default());
        assert_eq!(s, "flowchart TD\nA --> B");
    }

    #[test]
    fn frontmatter_title_and_theme() {
        let src = "---\ntitle: My Flow\nconfig:\n  theme: forest\n---\nflowchart TD\nA --> B\n";
        let (m, s) = strip(src);
        assert_eq!(m.title.as_deref(), Some("My Flow"));
        assert_eq!(m.theme.as_deref(), Some("forest"));
        assert_eq!(s, "flowchart TD\nA --> B");
    }

    #[test]
    fn quoted_frontmatter_title() {
        let (m, _) = strip("---\ntitle: \"Quoted: Title\"\n---\npie\n");
        assert_eq!(m.title.as_deref(), Some("Quoted: Title"));
    }

    #[test]
    fn init_directive_theme() {
        let src = "%%{init: {'theme': 'dark'}}%%\nflowchart TD\nA --> B\n";
        let (m, s) = strip(src);
        assert_eq!(m.theme.as_deref(), Some("dark"));
        assert_eq!(s, "flowchart TD\nA --> B");
    }

    #[test]
    fn acc_title_and_descr_line() {
        let src = "flowchart TD\naccTitle: The Title\naccDescr: The description\nA --> B\n";
        let (m, s) = strip(src);
        assert_eq!(m.acc_title.as_deref(), Some("The Title"));
        assert_eq!(m.acc_descr.as_deref(), Some("The description"));
        assert_eq!(s, "flowchart TD\nA --> B");
    }

    #[test]
    fn acc_descr_block() {
        let src = "flowchart TD\naccDescr {\n  line one\n  line two\n}\nA --> B\n";
        let (m, s) = strip(src);
        assert_eq!(m.acc_descr.as_deref(), Some("line one\nline two"));
        assert_eq!(s, "flowchart TD\nA --> B");
    }

    #[test]
    fn frontmatter_only_when_at_top() {
        // A `---` that is not the first content is not frontmatter.
        let src = "flowchart TD\n---\nA --> B\n";
        let (m, s) = strip(src);
        assert_eq!(m, DiagramMeta::default());
        assert_eq!(s, src.trim_end());
    }
}
