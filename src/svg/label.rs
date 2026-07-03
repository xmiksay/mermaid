//! Label text decoding shared by every renderer, applied by
//! [`SvgBuilder::text`][super::builder::SvgBuilder::text] before XML escaping.
//!
//! Ports the pre-render text handling upstream Mermaid does: resolving
//! `#`-prefixed entity codes. Backtick-fenced markdown *strings* and their
//! `**bold**`/`*italic*` emphasis are handled one layer up, in
//! [`markup::parse_spans`][super::markup::parse_spans], which emits styled
//! `<tspan>`s rather than flattening the emphasis to plain text.

/// Decode a display label the way upstream Mermaid does before rendering text:
/// resolve `#`-prefixed entity codes.
///
/// Mermaid uses `#` (not `&`) as the entity sentinel in diagram source, so
/// `#quot;` → `"`, `#35;` → `#`, `#9829;`/`#x2665;` → `♥`.
pub fn decode_label(s: &str) -> String {
    decode_entities(s)
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
    fn leaves_markdown_and_backticks_to_the_span_layer() {
        // decode_label no longer touches markdown fences/markers — that is the
        // span layer's job (markup::parse_spans). Bare `_`/`*` are never mangled.
        assert_eq!(decode_label("snake_case"), "snake_case");
        assert_eq!(decode_label("a * b"), "a * b");
    }
}
