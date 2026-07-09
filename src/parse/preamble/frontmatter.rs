//! Leading `--- … ---` YAML frontmatter extraction and its indentation-based
//! flattening into the dotted `config` map.

use super::super::ast::DiagramMeta;
use super::super::token::unquote_any as unquote;

/// Parse a leading `--- … ---` YAML frontmatter block, filling `title` and the
/// flattened `config` map. Returns the index of the first line *after* the
/// block (or 0 if there is no frontmatter).
pub(super) fn strip_frontmatter(lines: &[&str], meta: &mut DiagramMeta) -> usize {
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let Some(start) = first else { return 0 };
    if lines[start].trim() != "---" {
        return 0;
    }
    let Some(end) = lines[start + 1..].iter().position(|l| l.trim() == "---") else {
        return 0;
    };
    let end = start + 1 + end;

    for (dotted, value) in flatten_yaml(&lines[start + 1..end]) {
        if dotted == "title" {
            meta.title = Some(value);
        } else if let Some(key) = dotted.strip_prefix("config.") {
            meta.config.insert(key.to_string(), value);
        }
    }

    // Skip the closing `---` and any blank lines directly after it.
    let mut next = end + 1;
    while next < lines.len() && lines[next].trim().is_empty() {
        next += 1;
    }
    next
}

/// Flatten an indentation-structured YAML subset (nested maps of scalars) into
/// dotted `key → value` pairs. Only `key: value` and `key:` (map header) lines
/// are recognized — no lists or block scalars, which config never uses. Values
/// are unquoted; a bare value after the first `:` is kept verbatim (so a URL's
/// own `:` survives).
fn flatten_yaml(lines: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let (key, value) = match trimmed.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        if key.is_empty() {
            continue;
        }
        while stack.last().is_some_and(|(ind, _)| *ind >= indent) {
            stack.pop();
        }
        if value.is_empty() {
            stack.push((indent, key.to_string()));
        } else {
            let mut dotted = String::new();
            for (_, k) in &stack {
                dotted.push_str(k);
                dotted.push('.');
            }
            dotted.push_str(key);
            out.push((dotted, unquote(value).to_string()));
        }
    }
    out
}
