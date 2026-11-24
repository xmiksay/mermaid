//! Wrapping C4 text by its measured width, so no line is wider than the room
//! it is drawn in — including over-long words, URLs and identifiers, which
//! are broken after punctuation where possible and by character otherwise.

use super::super::metrics::proportional_width;

/// The text styles C4 draws, as `(font size, bold)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Font {
    Name,
    Techn,
    Descr,
    Label,
    LabelTech,
}

impl Font {
    fn style(self) -> (f64, bool) {
        match self {
            Font::Name => (13.0, true),
            Font::Techn | Font::Descr | Font::Label => (10.0, false),
            Font::LabelTech => (9.0, false),
        }
    }
}

/// Measured width of `s` in `font`.
pub(super) fn measure(s: &str, font: Font) -> f64 {
    let (size, bold) = font.style();
    proportional_width(s, size, bold)
}

/// A token may be broken after one of these, the mark staying on the upper
/// line (`api.example.com/` + `v2`).
const BREAK_AFTER: [char; 8] = ['.', '/', '-', '_', ':', '?', '&', '='];

/// Greedy word wrap of `s` into lines at most `max_w` wide in `font`.
pub(super) fn wrap(s: &str, max_w: f64, font: Font) -> Vec<String> {
    let fits = |t: &str| measure(t, font) <= max_w;
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let joined = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if fits(&joined) {
            cur = joined;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if fits(word) {
            cur = word.to_string();
            continue;
        }
        let mut pieces = break_token(word, &fits);
        cur = pieces.pop().unwrap_or_default();
        lines.extend(pieces);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Split a token wider than a line into pieces that each fit: at the last
/// break mark that keeps a reasonably full line, else by character.
fn break_token(word: &str, fits: &dyn Fn(&str) -> bool) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        // The longest piece from `start` that fits (always at least one char).
        let mut end = start + 1;
        while end < chars.len() && fits(&piece(start, end + 1)) {
            end += 1;
        }
        if end < chars.len() {
            let min_len = (end - start) / 3;
            if let Some(b) = (start + min_len.max(1)..end)
                .rev()
                .find(|&i| BREAK_AFTER.contains(&chars[i - 1]))
            {
                end = b;
            }
        }
        out.push(piece(start, end));
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_by_measured_width() {
        let lines = wrap(
            "WWW WWW WWW WWW WWW WWW WWW WWW WWW WWW",
            120.0,
            Font::Descr,
        );
        assert!(lines.len() > 1);
        assert!(
            lines.iter().all(|l| measure(l, Font::Descr) <= 120.0),
            "{lines:?}"
        );
        assert_eq!(lines.join(" "), "WWW WWW WWW WWW WWW WWW WWW WWW WWW WWW");
    }

    #[test]
    fn long_tokens_break_after_punctuation_then_by_character() {
        let url = "https://payments.internal.example.com/api/v2/charges";
        let lines = wrap(url, 120.0, Font::Descr);
        assert!(
            lines.iter().all(|l| measure(l, Font::Descr) <= 120.0),
            "{lines:?}"
        );
        assert_eq!(lines.concat(), url);
        // Every piece but the last ends on a break mark when one was in reach.
        assert!(
            lines[..lines.len() - 1]
                .iter()
                .all(|l| l.ends_with(BREAK_AFTER)),
            "{lines:?}"
        );
        let plain = "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMM";
        let lines = wrap(plain, 100.0, Font::Name);
        assert!(lines.len() > 1 && lines.iter().all(|l| measure(l, Font::Name) <= 100.0));
        assert_eq!(lines.concat(), plain);
    }

    #[test]
    fn wide_glyphs_wrap_too() {
        let cjk = "日本語のユーザー名がとても長いですから折り返す必要があります";
        let lines = wrap(cjk, 150.0, Font::Name);
        assert!(lines.len() > 1);
        assert!(
            lines.iter().all(|l| measure(l, Font::Name) <= 150.0),
            "{lines:?}"
        );
        assert_eq!(lines.concat(), cjk);
    }

    #[test]
    fn empty_text_has_no_lines() {
        assert!(wrap("", 100.0, Font::Descr).is_empty());
        assert!(wrap("   ", 100.0, Font::Descr).is_empty());
    }
}
