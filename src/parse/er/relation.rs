//! Relation-line parsing for the ER diagram parser: Crow's-Foot glyphs and the
//! verbal / numeric cardinality forms.

use crate::parse::ast::{Cardinality, ErRelation};
use crate::parse::token::{find_unquoted, split_id_label, unquote};

use super::split_style_class;

/// Parse a relation line, returning the relation plus any `(entity_id, class)`
/// assignments from a `:::class` style separator on either endpoint.
pub(super) fn parse_relation(line: &str) -> Option<(ErRelation, Vec<(String, String)>)> {
    // Split off the trailing `: label` first, skipping the `:::` style
    // separator so `A:::hot ||--o{ B : x` isn't cut at the wrong colon.
    let (spec, label) = match find_label_colon(line) {
        Some(i) => (line[..i].trim(), unquote(line[i + 1..].trim()).to_string()),
        None => (line.trim(), String::new()),
    };

    // Locate the relationship connector between the two cardinalities. It is
    // either a glyph line (`--`/`..`/`.-`/`-.`) or a verbal form
    // (`to`/`optionally to`).
    let (sep_start, sep_len, identifying) = find_reltype(spec)?;
    let left_part = spec[..sep_start].trim();
    let right_part = spec[sep_start + sep_len..].trim();

    let (left_name, left_card) = split_card_end(left_part)?;
    let (right_card, right_name) = split_card_start(right_part)?;
    if left_name.is_empty() || right_name.is_empty() {
        return None;
    }

    let (left_id, left_class) = split_entity_ref(left_name);
    let (right_id, right_class) = split_entity_ref(right_name);
    if left_id.is_empty() || right_id.is_empty() {
        return None;
    }

    let mut classes = Vec::new();
    if let Some(c) = left_class {
        classes.push((left_id.clone(), c));
    }
    if let Some(c) = right_class {
        classes.push((right_id.clone(), c));
    }

    Some((
        ErRelation {
            left: left_id,
            right: right_id,
            left_card,
            right_card,
            identifying,
            label,
        },
        classes,
    ))
}

/// Resolve an entity reference token — stripping a `:::class` separator, an
/// `[Alias]`, and surrounding quotes — into `(id, class)`.
fn split_entity_ref(token: &str) -> (String, Option<String>) {
    let (base, class) = split_style_class(token);
    let (id, _label) = split_id_label(base);
    (id, class.map(str::to_string))
}

/// First lone `:` in `line` (the `: label` separator), skipping any `:::`
/// style-separator run so its colons aren't mistaken for the label colon.
fn find_label_colon(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    (0..b.len()).find(|&i| {
        b[i] == b':' && !(i > 0 && b[i - 1] == b':') && !(i + 1 < b.len() && b[i + 1] == b':')
    })
}

/// Locate the relationship connector, returning `(byte offset, length,
/// identifying)`. Verbal forms take precedence over the glyph lines so an
/// `optionally to` isn't mistaken for a bare `to`. Scanning is quote-aware so
/// a quoted entity name embedding a glyph doesn't split the line. Only `--` is
/// identifying; `..`/`.-`/`-.` are all non-identifying (upstream lexer).
fn find_reltype(spec: &str) -> Option<(usize, usize, bool)> {
    const VERBAL: &[(&str, bool)] = &[(" optionally to ", false), (" to ", true)];
    for (tok, identifying) in VERBAL {
        if let Some(p) = find_unquoted(spec, tok) {
            return Some((p, tok.len(), *identifying));
        }
    }
    ["--", "..", ".-", "-."]
        .iter()
        .filter_map(|s| find_unquoted(spec, s).map(|p| (p, *s)))
        .min_by_key(|x| x.0)
        .map(|(p, s)| (p, s.len(), s == "--"))
}

/// Verbal / numeric cardinality aliases (upstream `erDiagram` grammar), matched
/// as whole words. Glyph tokens are handled separately.
const CARD_WORDS: &[(&str, Cardinality)] = &[
    ("only one", Cardinality::ExactlyOne),
    ("zero or one", Cardinality::ZeroOrOne),
    ("one or zero", Cardinality::ZeroOrOne),
    ("zero or more", Cardinality::ZeroOrMore),
    ("zero or many", Cardinality::ZeroOrMore),
    ("one or more", Cardinality::OneOrMore),
    ("one or many", Cardinality::OneOrMore),
    ("many(1)", Cardinality::OneOrMore),
    ("many(0)", Cardinality::ZeroOrMore),
    ("0+", Cardinality::ZeroOrMore),
    ("1+", Cardinality::OneOrMore),
    ("1", Cardinality::ExactlyOne),
];

/// Split `NAME <card>` where the cardinality sits at the END. Handles verbal /
/// numeric words as well as the crow's-foot glyphs.
fn split_card_end(s: &str) -> Option<(&str, Cardinality)> {
    let s = s.trim_end();
    for (tok, card) in CARD_WORDS {
        if let Some(name) = s.strip_suffix(tok) {
            if name.is_empty() || name.ends_with(char::is_whitespace) {
                return Some((name.trim_end(), *card));
            }
        }
    }
    const GLYPHS: &[(&str, Cardinality)] = &[
        ("}o", Cardinality::ZeroOrMore),
        ("}|", Cardinality::OneOrMore),
        ("o|", Cardinality::ZeroOrOne),
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
    ];
    for (tok, card) in GLYPHS {
        if let Some(name) = s.strip_suffix(tok) {
            return Some((name.trim_end(), *card));
        }
    }
    None
}

/// Split `<card> NAME` where the cardinality sits at the START.
fn split_card_start(s: &str) -> Option<(Cardinality, &str)> {
    let s = s.trim_start();
    for (tok, card) in CARD_WORDS {
        if let Some(name) = s.strip_prefix(tok) {
            if name.is_empty() || name.starts_with(char::is_whitespace) {
                return Some((*card, name.trim_start()));
            }
        }
    }
    const GLYPHS: &[(&str, Cardinality)] = &[
        ("o{", Cardinality::ZeroOrMore),
        ("|{", Cardinality::OneOrMore),
        ("o|", Cardinality::ZeroOrOne),
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
    ];
    for (tok, card) in GLYPHS {
        if let Some(name) = s.strip_prefix(tok) {
            return Some((*card, name.trim_start()));
        }
    }
    None
}
