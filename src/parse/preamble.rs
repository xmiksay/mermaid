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
use super::token::unquote_any as unquote;

/// Extract the preamble metadata and return `(meta, cleaned_source)`.
pub fn strip(input: &str) -> (DiagramMeta, String) {
    let mut meta = DiagramMeta::default();
    let lines: Vec<&str> = input.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());

    let mut i = strip_frontmatter(&lines, &mut meta);

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if let Some(inner) = init_inner(trimmed) {
            // A `%%{init}%%` directive line: consume it (theme + gitGraph keys
            // captured) and keep nothing — the per-diagram scanner would
            // otherwise see an empty comment line, which is harmless, but
            // dropping keeps the cleaned source tidy.
            apply_config_fragment(inner, &mut meta);
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
            } else if let Some(v) = strip_prefix_ci(trimmed, "ticketBaseUrl:") {
                meta.ticket_base_url = Some(unquote(v.trim()).to_string());
            } else if let Some(v) = strip_prefix_ci(trimmed, "valueFormat:") {
                meta.value_format = Some(unquote(v.trim()).to_string());
            } else if let Some(v) = strip_prefix_ci(trimmed, "mainBranchName:") {
                meta.git_graph.main_branch_name = Some(unquote(v.trim()).to_string());
            } else if let Some(v) = strip_prefix_ci(trimmed, "showBranches:") {
                meta.git_graph.show_branches = parse_flag(v.trim());
            } else if let Some(v) = strip_prefix_ci(trimmed, "showCommitLabel:") {
                meta.git_graph.show_commit_label = parse_flag(v.trim());
            } else if let Some(v) = strip_prefix_ci(trimmed, "rotateCommitLabel:") {
                meta.git_graph.rotate_commit_label = parse_flag(v.trim());
            } else if let Some(v) = strip_prefix_ci(trimmed, "parallelCommits:") {
                meta.git_graph.parallel_commits = parse_flag(v.trim());
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

/// If `line` is a `%%{init: … }%%` directive, return its inner fragment.
fn init_inner(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("%%{")?;
    let inner = inner.strip_suffix("%%").unwrap_or(inner);
    let inner = inner.strip_suffix("}").unwrap_or(inner);
    Some(inner)
}

/// Pull the config keys we honor out of a JSON-ish fragment (an init directive
/// body). `theme` and the `gitGraph.*` keys are searched by name; each is only
/// set when not already present so the first occurrence wins.
fn apply_config_fragment(inner: &str, meta: &mut DiagramMeta) {
    if meta.theme.is_none() {
        if let Some(t) = json_value(inner, "theme") {
            meta.theme = Some(t);
        }
    }
    let g = &mut meta.git_graph;
    if g.main_branch_name.is_none() {
        g.main_branch_name = json_value(inner, "mainBranchName");
    }
    if g.show_branches.is_none() {
        g.show_branches = json_value(inner, "showBranches").and_then(|v| parse_flag(&v));
    }
    if g.show_commit_label.is_none() {
        g.show_commit_label = json_value(inner, "showCommitLabel").and_then(|v| parse_flag(&v));
    }
    if g.rotate_commit_label.is_none() {
        g.rotate_commit_label = json_value(inner, "rotateCommitLabel").and_then(|v| parse_flag(&v));
    }
    if g.parallel_commits.is_none() {
        g.parallel_commits = json_value(inner, "parallelCommits").and_then(|v| parse_flag(&v));
    }
}

/// Parse a boolean flag value (`true`/`false`, plus common aliases).
fn parse_flag(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
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
    fn frontmatter_treemap_value_format() {
        let src =
            "---\nconfig:\n  treemap:\n    valueFormat: \"$0,0\"\n---\ntreemap-beta\n\"A\": 5\n";
        let (m, s) = strip(src);
        assert_eq!(m.value_format.as_deref(), Some("$0,0"));
        assert_eq!(s, "treemap-beta\n\"A\": 5");
    }

    #[test]
    fn init_directive_git_graph_config() {
        let src = "%%{init: {'gitGraph': {'mainBranchName': 'master', 'showCommitLabel': false, 'parallelCommits': true}}}%%\ngitGraph\ncommit\n";
        let (m, s) = strip(src);
        assert_eq!(m.git_graph.main_branch_name.as_deref(), Some("master"));
        assert_eq!(m.git_graph.show_commit_label, Some(false));
        assert_eq!(m.git_graph.parallel_commits, Some(true));
        assert_eq!(m.git_graph.show_branches, None);
        assert_eq!(s, "gitGraph\ncommit");
    }

    #[test]
    fn frontmatter_git_graph_config() {
        let src = "---\nconfig:\n  gitGraph:\n    mainBranchName: trunk\n    showBranches: false\n---\ngitGraph\ncommit\n";
        let (m, _) = strip(src);
        assert_eq!(m.git_graph.main_branch_name.as_deref(), Some("trunk"));
        assert_eq!(m.git_graph.show_branches, Some(false));
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
