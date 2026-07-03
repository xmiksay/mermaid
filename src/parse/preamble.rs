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
            // A `%%{init}%%` directive line: fold its config object into the
            // flattened map (first occurrence / frontmatter wins) and drop the
            // line so the per-diagram scanner never sees it.
            for (k, v) in parse_init_object(inner) {
                meta.config.entry(k).or_insert(v);
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

    derive_typed_fields(&mut meta);
    (meta, out.join("\n"))
}

/// Parse a leading `--- … ---` YAML frontmatter block, filling `title` and the
/// flattened `config` map. Returns the index of the first line *after* the
/// block (or 0 if there is no frontmatter).
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

/// Parse the config object of an `%%{init: {…}}%%` directive into flattened
/// dotted `key → value` pairs. The fragment is JSON-ish: single or double
/// quotes, bare keys/values, and nested objects (flattened with `.`). Parsing
/// starts at the first `{` (skipping the `init:` label).
fn parse_init_object(inner: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = inner.chars().collect();
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < chars.len() && chars[pos] != '{' {
        pos += 1;
    }
    parse_object(&chars, &mut pos, "", &mut out);
    out
}

/// Parse a `{ key: value, … }` object starting at `chars[*pos] == '{'`,
/// emitting `prefix`-qualified leaf entries. Recurses on nested objects.
fn parse_object(chars: &[char], pos: &mut usize, prefix: &str, out: &mut Vec<(String, String)>) {
    if *pos >= chars.len() || chars[*pos] != '{' {
        return;
    }
    *pos += 1; // consume '{'
    loop {
        skip_sep(chars, pos);
        if *pos >= chars.len() || chars[*pos] == '}' {
            *pos += 1; // consume '}' (or run off the end)
            return;
        }
        let Some(key) = parse_token(chars, pos, true) else {
            return;
        };
        skip_ws(chars, pos);
        if *pos >= chars.len() || chars[*pos] != ':' {
            return;
        }
        *pos += 1; // consume ':'
        skip_ws(chars, pos);
        if *pos < chars.len() && chars[*pos] == '{' {
            let nested = format!("{prefix}{key}.");
            parse_object(chars, pos, &nested, out);
        } else if let Some(value) = parse_token(chars, pos, false) {
            out.push((format!("{prefix}{key}"), value));
        }
    }
}

/// Read one quoted or bare token. A bare *key* stops at `:`/`,`/`}`/whitespace;
/// a bare *value* stops at `,`/`}` (trimmed) so multi-word values survive.
fn parse_token(chars: &[char], pos: &mut usize, is_key: bool) -> Option<String> {
    skip_ws(chars, pos);
    if *pos >= chars.len() {
        return None;
    }
    let c = chars[*pos];
    if c == '"' || c == '\'' {
        *pos += 1;
        let start = *pos;
        while *pos < chars.len() && chars[*pos] != c {
            *pos += 1;
        }
        let s: String = chars[start..*pos].iter().collect();
        if *pos < chars.len() {
            *pos += 1; // consume closing quote
        }
        return Some(s);
    }
    let start = *pos;
    while *pos < chars.len() {
        let c = chars[*pos];
        if c == ',' || c == '}' || (is_key && (c == ':' || c.is_whitespace())) {
            break;
        }
        *pos += 1;
    }
    let s: String = chars[start..*pos].iter().collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn skip_ws(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

/// Skip whitespace and separators (`,`) between object entries.
fn skip_sep(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && (chars[*pos].is_whitespace() || chars[*pos] == ',') {
        *pos += 1;
    }
}

/// Populate the typed [`DiagramMeta`] fields the renderer honors from the
/// flattened `config` map.
fn derive_typed_fields(meta: &mut DiagramMeta) {
    let get = |k: &str| meta.config.get(k).cloned();

    meta.theme = get("theme");
    meta.font_family = get("fontFamily");
    meta.font_size = get("fontSize").as_deref().and_then(parse_font_size);
    meta.use_max_width = get("useMaxWidth").as_deref().and_then(parse_flag);
    meta.look = get("look");
    meta.layout = get("layout");
    meta.security_level = get("securityLevel");
    meta.ticket_base_url = get("kanban.ticketBaseUrl");
    meta.value_format = get("treemap.valueFormat");
    meta.show_values = get("treemap.showValues").as_deref().and_then(parse_flag);
    meta.sankey_link_color = get("sankey.linkColor");
    meta.sankey_node_alignment = get("sankey.nodeAlignment");

    for (k, v) in &meta.config {
        if let Some(name) = k.strip_prefix("themeVariables.") {
            meta.theme_variables.insert(name.to_string(), v.clone());
        }
    }

    let g = &mut meta.git_graph;
    g.main_branch_name = meta.config.get("gitGraph.mainBranchName").cloned();
    g.show_branches = meta
        .config
        .get("gitGraph.showBranches")
        .and_then(|v| parse_flag(v));
    g.show_commit_label = meta
        .config
        .get("gitGraph.showCommitLabel")
        .and_then(|v| parse_flag(v));
    g.rotate_commit_label = meta
        .config
        .get("gitGraph.rotateCommitLabel")
        .and_then(|v| parse_flag(v));
    g.parallel_commits = meta
        .config
        .get("gitGraph.parallelCommits")
        .and_then(|v| parse_flag(v));
}

/// Parse a `fontSize` value that may carry a `px` suffix (`"16px"` / `"16"`).
fn parse_font_size(s: &str) -> Option<f64> {
    let s = s.trim();
    let num = s.strip_suffix("px").unwrap_or(s).trim();
    num.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

/// Parse a boolean flag value (`true`/`false`, plus common aliases).
fn parse_flag(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// Case-insensitive `strip_prefix`.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// Read-only view of the flattened config map for the tests below.
#[cfg(test)]
fn config_of(src: &str) -> std::collections::BTreeMap<String, String> {
    strip(src).0.config
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
    fn frontmatter_theme_variables() {
        let src = "---\nconfig:\n  theme: base\n  themeVariables:\n    primaryColor: \"#ff0000\"\n    lineColor: \"#00ff00\"\n---\nflowchart TD\nA --> B\n";
        let (m, _) = strip(src);
        assert_eq!(m.theme.as_deref(), Some("base"));
        assert_eq!(
            m.theme_variables.get("primaryColor").map(String::as_str),
            Some("#ff0000")
        );
        assert_eq!(
            m.theme_variables.get("lineColor").map(String::as_str),
            Some("#00ff00")
        );
    }

    #[test]
    fn init_directive_nested_theme_variables() {
        let src = "%%{init: {'theme': 'base', 'themeVariables': {'primaryColor': '#abcdef'}}}%%\nflowchart TD\nA --> B\n";
        let (m, _) = strip(src);
        assert_eq!(m.theme.as_deref(), Some("base"));
        assert_eq!(
            m.theme_variables.get("primaryColor").map(String::as_str),
            Some("#abcdef")
        );
    }

    #[test]
    fn font_family_and_use_max_width() {
        let src = "---\nconfig:\n  fontFamily: \"Courier New\"\n  fontSize: 18\n  useMaxWidth: false\n---\npie\n\"A\": 1\n";
        let (m, _) = strip(src);
        assert_eq!(m.font_family.as_deref(), Some("Courier New"));
        assert_eq!(m.font_size, Some(18.0));
        assert_eq!(m.use_max_width, Some(false));
    }

    #[test]
    fn generic_config_map_captures_per_diagram_keys() {
        let cfg = config_of("---\nconfig:\n  flowchart:\n    htmlLabels: false\n    curve: linear\n---\nflowchart TD\nA --> B\n");
        assert_eq!(
            cfg.get("flowchart.htmlLabels").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            cfg.get("flowchart.curve").map(String::as_str),
            Some("linear")
        );
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
    fn frontmatter_treemap_show_values() {
        let src = "---\nconfig:\n  treemap:\n    showValues: false\n---\ntreemap-beta\n\"A\": 5\n";
        let (m, _) = strip(src);
        assert_eq!(m.show_values, Some(false));
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
    fn kanban_ticket_base_url() {
        let src = "---\nconfig:\n  kanban:\n    ticketBaseUrl: 'https://example.com/#TICKET#'\n---\nkanban\n";
        let (m, _) = strip(src);
        assert_eq!(
            m.ticket_base_url.as_deref(),
            Some("https://example.com/#TICKET#")
        );
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
