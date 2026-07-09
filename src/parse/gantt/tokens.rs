//! Token classification helpers: directive keywords, date/start/end shapes,
//! and duration parsing.

/// Strip a bare directive keyword, tolerating a space and/or a `:` separator
/// (`weekend friday`, `displayMode: compact`). Returns the trimmed argument,
/// or `None` when the keyword isn't a standalone token (so `displayModern`
/// doesn't match `displayMode`).
pub(super) fn strip_kw<'a>(line: &'a str, kw: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(kw)?;
    match rest.chars().next() {
        Some(c) if !(c.is_whitespace() || c == ':') => None,
        _ => Some(rest.trim_start().trim_start_matches(':').trim()),
    }
}

pub(super) fn looks_like_date(s: &str) -> bool {
    // A calendar date has ≥2 `-` separators (`YYYY-MM-DD`, `DD-MM-YYYY`); a
    // sub-day time token (`HH:mm`, `HH:mm:ss`) is digits joined by `:`. Either
    // way it carries separators, so it's never mistaken for a bare id like `a1`.
    s.chars().filter(|c| *c == '-').count() >= 2
        || (s.contains(':') && s.chars().all(|c| c.is_ascii_digit() || c == ':'))
}

pub(super) fn looks_like_start(s: &str) -> bool {
    s.starts_with("after ") || looks_like_date(s)
}

/// A token that ends a task: a duration, `until <id>`, or an end date.
pub(super) fn looks_like_end(s: &str) -> bool {
    parse_duration(s).is_some() || s.starts_with("until ") || looks_like_date(s)
}

/// A duration token → its length in days. Upstream units `ms`/`s`/`m`/`h`/`d`/
/// `w`/`M`/`y` (decimals allowed); `M`(onth) and `y`(ear) are approximated as
/// 30 and 365 days for the day-count model. `ms` is matched before the
/// single-char `m`/`s` so it isn't mis-read.
pub(super) fn parse_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    let (num_part, unit) = if let Some(rest) = s.strip_suffix("ms") {
        (rest, 1.0 / 86_400_000.0)
    } else if let Some(rest) = s.strip_suffix('s') {
        (rest, 1.0 / 86_400.0)
    } else if let Some(rest) = s.strip_suffix('m') {
        (rest, 1.0 / 1_440.0)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest, 1.0 / 24.0)
    } else if let Some(rest) = s.strip_suffix('d') {
        (rest, 1.0)
    } else if let Some(rest) = s.strip_suffix('w') {
        (rest, 7.0)
    } else if let Some(rest) = s.strip_suffix('M') {
        (rest, 30.0)
    } else if let Some(rest) = s.strip_suffix('y') {
        (rest, 365.0)
    } else {
        return None;
    };
    num_part.parse::<f64>().ok().map(|n| n * unit)
}
