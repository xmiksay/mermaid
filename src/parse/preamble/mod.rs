//! Shared source *preamble* handling, run before per-diagram dispatch.
//!
//! Upstream Mermaid lets any diagram carry a chunk of cross-cutting metadata
//! that is not part of the diagram body:
//!
//! - **YAML frontmatter** delimited by `---` lines at the very top, carrying a
//!   `title:` and a nested `config:` block.
//! - **`%%{init: {...}}%%` init directives** anywhere in the source.
//! - **`accTitle:` / `accDescr:`** accessibility statements (the latter also in
//!   a `accDescr { … }` block form).
//!
//! The whole `config:` tree (frontmatter *and* init) is flattened into
//! [`DiagramMeta::config`], a dotted `key → value` map (e.g.
//! `themeVariables.primaryColor`, `gitGraph.mainBranchName`,
//! `kanban.ticketBaseUrl`). The typed [`DiagramMeta`] fields the renderer
//! honors are then derived from that map, so adding support for another config
//! key is a lookup, not new scanning.
//!
//! [`strip`] pulls all of the above out of the raw source, returning the
//! extracted [`DiagramMeta`] plus a cleaned source string with the preamble
//! removed so the line-oriented per-diagram scanners never see it.

mod config;
mod frontmatter;
mod init;
#[cfg(test)]
mod tests;

use super::ast::DiagramMeta;
use config::derive_typed_fields;
use frontmatter::strip_frontmatter;
use init::{collect_init, parse_init_object};

/// Extract the preamble metadata and return `(meta, cleaned_source)`.
pub fn strip(input: &str) -> (DiagramMeta, String) {
    let mut meta = DiagramMeta::default();
    let lines: Vec<&str> = input.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());

    let mut i = strip_frontmatter(&lines, &mut meta);

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("%%{") {
            // A `%%{init}%%` directive, possibly wrapping across lines (upstream's
            // directiveRegex spans newlines). Fold its config object into the
            // flattened map — a directive overrides frontmatter and the last init
            // wins (upstream cleanAndMerge / assignWithDepth), so plain `insert`
            // (last write wins) — and drop the whole directive so the per-diagram
            // scanner never sees it.
            if let Some((inner, next)) = collect_init(&lines, i) {
                for (k, v) in parse_init_object(&inner) {
                    meta.config.insert(k, v);
                }
                i = next;
                continue;
            }
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

    derive_typed_fields(&mut meta);
    (meta, out.join("\n"))
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

/// Case-insensitive `strip_prefix`. The boundary check keeps multibyte input
/// from panicking the slice: a non-boundary cut can never match an ASCII
/// prefix anyway.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix)
    {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}
