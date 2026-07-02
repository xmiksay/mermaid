//! Label text decoding shared by every renderer, applied by
//! [`SvgBuilder::text`][super::builder::SvgBuilder::text] before XML escaping.
//!
//! Ports the pre-render text handling upstream Mermaid does: resolving
//! `#`-prefixed entity codes and stripping lightweight markdown emphasis from
//! backtick-fenced markdown *strings*.

/// Decode a display label the way upstream Mermaid does before rendering text:
/// resolve `#`-prefixed entity codes and strip lightweight markdown emphasis.
///
/// Mermaid uses `#` (not `&`) as the entity sentinel in diagram source, so
/// `#quot;` → `"`, `#35;` → `#`, `#9829;`/`#x2665;` → `♥`. Markdown *strings*
/// (`"`**bold**`"`) survive parsing as a backtick-fenced value; we drop the
/// fence and the `**`/`__`/`*`/`_` emphasis markers, rendering the plain text
/// (a partial port — no bold/italic styling yet).
pub fn decode_label(s: &str) -> String {
    strip_markdown(&decode_entities(s))
}

fn decode_entities(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            if let Some((decoded, len)) = decode_entity(&s[i..]) {
                out.push_str(&decoded);
                i += len;
                continue;
            }
        }
        // Push one UTF-8 char, not one byte, to stay valid on multibyte input.
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Decode a single `#…;` entity at the start of `s`, returning `(text, len)`.
fn decode_entity(s: &str) -> Option<(String, usize)> {
    let body = s.strip_prefix('#')?;
    let end = body.find(';')?;
    let token = &body[..end];
    if token.is_empty() || token.len() > 10 {
        return None;
    }
    let len = 1 + end + 1; // '#' + token + ';'
    let ch = if let Some(hex) = token.strip_prefix(['x', 'X']) {
        char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
    } else if token.bytes().all(|c| c.is_ascii_digit()) {
        char::from_u32(token.parse().ok()?)?
    } else {
        match token {
            "quot" => '"',
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "apos" => '\'',
            "nbsp" => '\u{a0}',
            "hearts" => '♥',
            "colon" => ':',
            "semi" => ';',
            _ => return None,
        }
    };
    Some((ch.to_string(), len))
}

/// For a backtick-fenced markdown *string*, drop the fence and inline
/// `**`/`__`/`*`/`_` emphasis markers, leaving the plain text. Non-fenced input
/// is returned untouched, so ordinary labels containing `_` or `*` (e.g.
/// `snake_case`, `a*b`) are never mangled.
fn strip_markdown(s: &str) -> String {
    let trimmed = s.trim();
    let Some(inner) = trimmed.strip_prefix('`').and_then(|t| t.strip_suffix('`')) else {
        return s.to_string();
    };
    let mut out = String::with_capacity(inner.len());
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if (bytes[i] == b'*' || bytes[i] == b'_') && i + 1 < bytes.len() && bytes[i + 1] == bytes[i]
        {
            i += 2; // `**` / `__`
        } else if bytes[i] == b'*' || bytes[i] == b'_' {
            i += 1; // `*` / `_`
        } else {
            let ch = inner[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_entity_codes() {
        assert_eq!(decode_label("a #quot;b#quot; c"), "a \"b\" c");
        assert_eq!(decode_label("#35; hash"), "# hash");
        assert_eq!(decode_label("#9829; and #x2665;"), "♥ and ♥");
        assert_eq!(decode_label("a #lt; b #gt; c"), "a < b > c");
        // Not an entity: left untouched.
        assert_eq!(decode_label("C#Sharp"), "C#Sharp");
        assert_eq!(decode_label("#notclosed"), "#notclosed");
    }

    #[test]
    fn strips_markdown_only_when_fenced() {
        assert_eq!(decode_label("`**bold**`"), "bold");
        assert_eq!(decode_label("`a *b* c`"), "a b c");
        // Bare labels with `_`/`*` are never mangled.
        assert_eq!(decode_label("snake_case"), "snake_case");
        assert_eq!(decode_label("a * b"), "a * b");
    }
}
