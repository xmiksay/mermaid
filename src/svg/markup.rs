//! Inline-HTML label markup, shared by [`SvgBuilder::text`].
//!
//! Upstream Mermaid defaults to `htmlLabels: true`, so labels routinely carry
//! inline HTML (`<b>`, `<i>`, `<u>`, `<span style="color:…">`, `<a href>`). A
//! static SVG renderer can't use `foreignObject` portably, but a small
//! whitelist maps cleanly onto styled `<tspan>`s in the existing multi-line
//! machinery:
//!
//! - `<b>`/`<strong>` → `font-weight="bold"`
//! - `<i>`/`<em>`     → `font-style="italic"`
//! - `<u>`            → `text-decoration="underline"`
//! - `<span style="color:…">` → `fill="…"`
//! - `<a href="…">`   → wrap the run in an SVG `<a>` link
//!
//! Unknown tags are **stripped** (not escaped), so output degrades to plain
//! text instead of tag soup. Tag scanning runs on the raw source *before*
//! entity decoding so `#lt;`-encoded angle brackets never masquerade as tags.

use super::label::decode_label;

/// A run of label text carrying its inline styling. A plain label decodes to a
/// single unstyled span, preserving the pre-existing single-`<text>` fast path.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Option<String>,
    pub href: Option<String>,
}

impl Span {
    /// True when the run carries no styling — eligible for the plain fast path.
    pub fn is_plain(&self) -> bool {
        !self.bold && !self.italic && !self.underline && self.color.is_none() && self.href.is_none()
    }

    fn styled_like(&self, s: &Style) -> bool {
        self.bold == s.bold
            && self.italic == s.italic
            && self.underline == s.underline
            && self.color == s.color
            && self.href == s.href
    }
}

#[derive(Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<String>,
    href: Option<String>,
}

/// Parse a run of already-`<br>`/newline-split label lines into per-line styled
/// runs, carrying the open-tag style stack **across** the line breaks so a tag
/// opened on one line still styles the next: `<b>a<br>b</b>` keeps both lines
/// bold (#187). Each line is parsed independently only for backtick-fenced
/// markdown, which is self-contained.
pub(crate) fn parse_lines(lines: &[&str]) -> Vec<Vec<Span>> {
    let mut cur = Style::default();
    let mut stack: Vec<(String, Style)> = Vec::new();
    lines
        .iter()
        .map(|line| parse_line(line, &mut cur, &mut stack))
        .collect()
}

/// Parse one already-line-split label into styled runs. Adjacent runs sharing
/// the same style are merged so a tag-free label yields exactly one span. The
/// library drives whole labels through [`parse_lines`]; this single-line form is
/// kept for the unit tests that exercise span parsing in isolation.
#[cfg(test)]
pub(crate) fn parse_spans(line: &str) -> Vec<Span> {
    let mut cur = Style::default();
    let mut stack: Vec<(String, Style)> = Vec::new();
    parse_line(line, &mut cur, &mut stack)
}

/// Parse one line, threading the caller's `cur` style and open-tag `stack` so a
/// tag left open at the line's end continues into the next line's parse.
fn parse_line(line: &str, cur: &mut Style, stack: &mut Vec<(String, Style)>) -> Vec<Span> {
    // Backtick-fenced markdown strings carry `**bold**`/`*italic*` emphasis
    // rather than HTML tags — parse those into styled runs instead. Such a line
    // is self-contained, so it neither reads nor mutates the carried tag stack.
    if let Some(inner) = fenced_markdown(line) {
        return parse_markdown_spans(inner);
    }
    let b = line.as_bytes();
    let mut spans: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            if let Some((tag, len)) = parse_tag(&line[i..]) {
                flush(&mut spans, &mut buf, cur);
                apply_tag(&tag, cur, stack);
                i += len;
                continue;
            }
        }
        let ch = line[i..].chars().next().unwrap();
        buf.push(ch);
        i += ch.len_utf8();
    }
    flush(&mut spans, &mut buf, cur);
    if spans.is_empty() {
        spans.push(Span::default());
    }
    spans
}

/// If `line` (trimmed) is a backtick-fenced markdown *string*, return its inner
/// text. Mermaid marks markdown-string labels by wrapping them in backticks.
fn fenced_markdown(line: &str) -> Option<&str> {
    line.trim().strip_prefix('`')?.strip_suffix('`')
}

/// Parse a markdown-string body into styled runs: `**`/`__` toggle bold, `*`/`_`
/// toggle italic. Marker-free text yields a single plain run, so a fenced label
/// with no emphasis still renders as bare `<text>`.
fn parse_markdown_spans(inner: &str) -> Vec<Span> {
    let b = inner.as_bytes();
    let mut spans: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut cur = Style::default();
    let mut i = 0;
    while i < b.len() {
        if (b[i] == b'*' || b[i] == b'_') && i + 1 < b.len() && b[i + 1] == b[i] {
            flush(&mut spans, &mut buf, &cur);
            cur.bold = !cur.bold;
            i += 2;
        } else if b[i] == b'*' || b[i] == b'_' {
            flush(&mut spans, &mut buf, &cur);
            cur.italic = !cur.italic;
            i += 1;
        } else {
            let ch = inner[i..].chars().next().unwrap();
            buf.push(ch);
            i += ch.len_utf8();
        }
    }
    flush(&mut spans, &mut buf, &cur);
    if spans.is_empty() {
        spans.push(Span::default());
    }
    spans
}

/// Concatenate a label's visible text with every HTML tag removed, leaving
/// entity codes untouched. Used for width estimation, so a tag-free label
/// measures byte-identically to before this feature existed.
pub(crate) fn strip_tags(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            if let Some((_, len)) = parse_tag(&s[i..]) {
                i += len;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn flush(spans: &mut Vec<Span>, buf: &mut String, cur: &Style) {
    if buf.is_empty() {
        return;
    }
    let text = decode_label(buf);
    buf.clear();
    if let Some(last) = spans.last_mut() {
        if last.styled_like(cur) {
            last.text.push_str(&text);
            return;
        }
    }
    spans.push(Span {
        text,
        bold: cur.bold,
        italic: cur.italic,
        underline: cur.underline,
        color: cur.color.clone(),
        href: cur.href.clone(),
    });
}

struct Tag {
    close: bool,
    self_closing: bool,
    name: String,
    attrs: String,
}

/// Scan a single HTML tag at the start of `s`, returning it plus the byte
/// length consumed, or `None` when `<` does not begin a well-formed tag (a bare
/// `<` is then kept as literal text). The grammar is deliberately small: `<`,
/// an optional `/`, an ASCII-letter-led name, arbitrary attribute text, an
/// optional `/`, and `>`.
fn parse_tag(s: &str) -> Option<(Tag, usize)> {
    let b = s.as_bytes();
    if b.is_empty() || b[0] != b'<' {
        return None;
    }
    let mut j = 1;
    let close = j < b.len() && b[j] == b'/';
    if close {
        j += 1;
    }
    let name_start = j;
    if j >= b.len() || !b[j].is_ascii_alphabetic() {
        return None;
    }
    while j < b.len() && b[j].is_ascii_alphanumeric() {
        j += 1;
    }
    let name = s[name_start..j].to_string();
    // Attributes run to the first unquoted `>`; a quote pauses the scan so a
    // `>` inside an attribute value (e.g. `title="a>b"`) doesn't close early.
    let attr_start = j;
    let mut quote: Option<u8> = None;
    while j < b.len() {
        let c = b[j];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == b'"' || c == b'\'' {
                    quote = Some(c);
                } else if c == b'>' {
                    break;
                }
            }
        }
        j += 1;
    }
    if j >= b.len() {
        return None; // unterminated tag → treat `<` as literal
    }
    let mut attrs = s[attr_start..j].trim();
    let self_closing = attrs.ends_with('/');
    if self_closing {
        attrs = attrs[..attrs.len() - 1].trim_end();
    }
    Some((
        Tag {
            close,
            self_closing,
            name,
            attrs: attrs.to_string(),
        },
        j + 1,
    ))
}

fn apply_tag(tag: &Tag, cur: &mut Style, stack: &mut Vec<(String, Style)>) {
    let name = tag.name.to_ascii_lowercase();
    if tag.close {
        if let Some(pos) = stack.iter().rposition(|(n, _)| *n == name) {
            *cur = stack[pos].1.clone();
            stack.truncate(pos);
        }
        return;
    }
    let saved = cur.clone();
    let recognized = match name.as_str() {
        "b" | "strong" => {
            cur.bold = true;
            true
        }
        "i" | "em" => {
            cur.italic = true;
            true
        }
        "u" => {
            cur.underline = true;
            true
        }
        "span" => {
            if let Some(c) = style_color(&tag.attrs) {
                cur.color = Some(c);
            }
            true
        }
        "a" => {
            if let Some(h) = attr_value(&tag.attrs, "href") {
                cur.href = Some(h);
            }
            true
        }
        _ => false, // unknown tag: stripped, styling unchanged
    };
    // A self-closing recognized tag (`<b/>`) styles nothing and needs no close;
    // an unknown tag never enters the stack, so its stray close is ignored.
    if recognized && !tag.self_closing {
        stack.push((name, saved));
    } else if tag.self_closing {
        *cur = saved;
    }
}

/// Extract `color:<value>` from a `style="…"` attribute run.
fn style_color(attrs: &str) -> Option<String> {
    let style = attr_value(attrs, "style")?;
    for decl in style.split(';') {
        let (k, v) = decl.split_once(':')?;
        if k.trim().eq_ignore_ascii_case("color") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Read the value of attribute `key` from a tag's attribute run, accepting
/// double- or single-quoted values (`key="v"` / `key='v'`).
fn attr_value(attrs: &str, key: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(key) {
        let at = from + rel;
        // Require a word boundary before the key so `href` doesn't match inside
        // another attribute name.
        let boundary = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = &attrs[at + key.len()..];
        let trimmed = after.trim_start();
        if boundary && trimmed.starts_with('=') {
            let rest = trimmed[1..].trim_start();
            let mut chars = rest.chars();
            let quote = chars.next()?;
            if quote == '"' || quote == '\'' {
                let val = chars.as_str();
                let end = val.find(quote)?;
                return Some(val[..end].to_string());
            }
        }
        from = at + key.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> Span {
        Span {
            text: text.to_string(),
            ..Span::default()
        }
    }

    #[test]
    fn plain_label_is_single_unstyled_span() {
        assert_eq!(parse_spans("hello world"), vec![plain("hello world")]);
        assert!(parse_spans("hello")[0].is_plain());
    }

    #[test]
    fn bold_italic_underline() {
        assert_eq!(
            parse_spans("<b>x</b>"),
            vec![Span {
                text: "x".into(),
                bold: true,
                ..Span::default()
            }]
        );
        assert!(parse_spans("<i>x</i>")[0].italic);
        assert!(parse_spans("<em>x</em>")[0].italic);
        assert!(parse_spans("<strong>x</strong>")[0].bold);
        assert!(parse_spans("<u>x</u>")[0].underline);
    }

    #[test]
    fn mixed_and_nested_runs() {
        let spans = parse_spans("a<b>b<i>c</i></b>d");
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0], plain("a"));
        assert!(spans[1].bold && !spans[1].italic);
        assert!(spans[2].bold && spans[2].italic);
        assert_eq!(spans[3], plain("d"));
    }

    #[test]
    fn span_color_and_link() {
        let s = parse_spans("<span style=\"color:red\">r</span>");
        assert_eq!(s[0].color.as_deref(), Some("red"));
        let a = parse_spans("<a href=\"https://x\">y</a>");
        assert_eq!(a[0].href.as_deref(), Some("https://x"));
    }

    #[test]
    fn unknown_tags_are_stripped_not_escaped() {
        // `<div>`/`<script>` are unknown → removed, text kept and merged.
        assert_eq!(parse_spans("a<div>b</div>c"), vec![plain("abc")]);
        assert_eq!(parse_spans("<script>x</script>y"), vec![plain("xy")]);
    }

    #[test]
    fn bare_angle_brackets_stay_literal() {
        // A `<` that doesn't open a tag is kept verbatim (decoded later).
        assert_eq!(parse_spans("a < b"), vec![plain("a < b")]);
        assert_eq!(parse_spans("x <"), vec![plain("x <")]);
    }

    #[test]
    fn entities_decode_inside_runs() {
        // `#gt;` must not be seen as a tag; it decodes after tag scanning.
        assert_eq!(parse_spans("a #lt;b#gt; c"), vec![plain("a <b> c")]);
        assert_eq!(parse_spans("<b>#hearts;</b>")[0].text, "♥");
    }

    #[test]
    fn strip_tags_leaves_plain_labels_untouched() {
        assert_eq!(strip_tags("hello #gt; world"), "hello #gt; world");
        assert_eq!(strip_tags("a < b"), "a < b");
        assert_eq!(strip_tags("<b>bold</b> and <i>it</i>"), "bold and it");
    }

    #[test]
    fn markdown_string_emphasis_becomes_styled_spans() {
        let bold = parse_spans("`**bold**`");
        assert_eq!(
            bold,
            vec![Span {
                text: "bold".into(),
                bold: true,
                ..Span::default()
            }]
        );
        let italic = parse_spans("`*it*`");
        assert!(italic[0].italic && !italic[0].bold);
        // Underscore forms toggle the same way.
        assert!(parse_spans("`__b__`")[0].bold);
        assert!(parse_spans("`_i_`")[0].italic);
    }

    #[test]
    fn markdown_string_mixes_plain_and_emphasis() {
        let spans = parse_spans("`a **b** c`");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0], plain("a "));
        assert!(spans[1].bold && spans[1].text == "b");
        assert_eq!(spans[2], plain(" c"));
    }

    #[test]
    fn markdown_string_nests_bold_and_italic() {
        let spans = parse_spans("`**b _bi_**`");
        assert!(spans[0].bold && !spans[0].italic);
        let bi = spans.iter().find(|s| s.text == "bi").unwrap();
        assert!(bi.bold && bi.italic);
    }

    #[test]
    fn plain_fenced_string_is_single_span() {
        // No emphasis markers → one plain run, so the bare-<text> fast path holds.
        assert_eq!(parse_spans("`plain`"), vec![plain("plain")]);
    }

    #[test]
    fn tag_stack_carries_across_lines() {
        // #187: <b> opened on the first line still styles the second line.
        let lines = ["<b>line1", "line2</b>", "plain"];
        let parsed = parse_lines(&lines);
        assert!(parsed[0][0].bold);
        assert!(parsed[1][0].bold);
        assert!(!parsed[2][0].bold);
    }

    #[test]
    fn quoted_gt_in_attr_does_not_close_early() {
        let s = parse_spans("<span style=\"color:red\" title=\"a>b\">z</span>");
        assert_eq!(s[0].text, "z");
        assert_eq!(s[0].color.as_deref(), Some("red"));
    }
}
