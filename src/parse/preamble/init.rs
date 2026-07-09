//! `%%{init: {…}}%%` directive collection and its JSON-ish object parsing into
//! flattened dotted `key → value` pairs.

/// Collect a possibly multi-line `%%{init: … }%%` directive starting at line
/// `i` (whose trimmed text opens with `%%{`). Upstream's directive regex spans
/// newlines, so a pretty-printed init object may wrap across several lines.
/// Returns the joined inner fragment and the index just past the closing `}%%`.
pub(super) fn collect_init(lines: &[&str], i: usize) -> Option<(String, usize)> {
    let mut joined = String::new();
    let mut j = i;
    while j < lines.len() {
        if !joined.is_empty() {
            joined.push('\n');
        }
        joined.push_str(lines[j]);
        j += 1;
        if joined.contains("}%%") {
            return Some((init_inner(joined.trim())?.to_string(), j));
        }
    }
    None
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
pub(super) fn parse_init_object(inner: &str) -> Vec<(String, String)> {
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
