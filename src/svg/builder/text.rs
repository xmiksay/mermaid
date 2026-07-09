//! Label text helpers: line splitting and XML escaping.

/// Split a label into display lines, honoring the line breaks that upstream
/// Mermaid recognizes: HTML `<br>` / `<br/>` / `<br />` (case-insensitive, with
/// optional inner whitespace) and `\n` — the latter as a real newline or the
/// two-character literal escape `\n` that survives in Mermaid source. Each line
/// is trimmed of surrounding whitespace. A label with no breaks yields a single
/// element, so callers keep their existing single-line behavior.
pub fn split_label_lines(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut lines: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\n' {
            lines.push(s[start..i].trim());
            i += 1;
            start = i;
        } else if b[i] == b'\\' && i + 1 < b.len() && b[i + 1] == b'n' {
            lines.push(s[start..i].trim());
            i += 2;
            start = i;
        } else if let Some(len) = match_br(&b[i..]) {
            lines.push(s[start..i].trim());
            i += len;
            start = i;
        } else {
            i += 1;
        }
    }
    lines.push(s[start..].trim());
    lines
}

/// Length of a `<br>`-style tag at the start of `b`, or `None`. Matches `<br`
/// then optional whitespace, an optional `/`, more optional whitespace, and `>`.
fn match_br(b: &[u8]) -> Option<usize> {
    if b.len() < 4 || b[0] != b'<' {
        return None;
    }
    if !b[1].eq_ignore_ascii_case(&b'b') || !b[2].eq_ignore_ascii_case(&b'r') {
        return None;
    }
    let mut j = 3;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j < b.len() && b[j] == b'/' {
        j += 1;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
    }
    if j < b.len() && b[j] == b'>' {
        Some(j + 1)
    } else {
        None
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
