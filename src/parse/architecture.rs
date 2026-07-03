//! architecture-beta parser.
//!
//! Grammar:
//!
//! ```text
//! architecture-beta
//!     group api(cloud)[API]
//!     service db(database)[Database] in api
//!     service disk1(disk)[Storage] in api
//!     junction junction1
//!     db:L -- R:disk1
//!     db:T --> B:disk1
//! ```

use super::ast::{ArchEdge, ArchGroup, ArchJunction, ArchService, ArchSide, ArchitectureDiagram};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<ArchitectureDiagram, ParseError> {
    let mut d = ArchitectureDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "architecture-beta" && line != "architecture" {
                return Err(ParseError::header(
                    line_no,
                    "expected 'architecture-beta' header",
                ));
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("group") {
            d.groups.push(parse_group(rest, line_no)?);
        } else if let Some(rest) = line.strip_prefix("service") {
            d.services.push(parse_service(rest, line_no)?);
        } else if let Some(rest) = line.strip_prefix("junction") {
            let (id, parent) = split_parent(rest);
            d.junctions.push(ArchJunction { id, parent });
        } else if is_align_stmt(line) {
            // v11.16+ `align row|column id id...` — consumed; honoring the
            // alignment constraint in layout is a follow-up.
        } else {
            d.edges.push(parse_edge(line, line_no)?);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_group(rest: &str, _line_no: usize) -> Result<ArchGroup, ParseError> {
    let (head, parent) = split_parent(rest);
    let (id, icon, label) = parse_id_icon_label(&head);
    Ok(ArchGroup {
        id,
        icon,
        label,
        parent,
    })
}

fn parse_service(rest: &str, _line_no: usize) -> Result<ArchService, ParseError> {
    let (head, parent) = split_parent(rest);
    let (id, icon, label) = parse_id_icon_label(&head);
    Ok(ArchService {
        id,
        icon,
        label,
        parent,
    })
}

fn split_parent(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    if let Some((head, p)) = s.rsplit_once(" in ") {
        (head.trim().to_string(), Some(p.trim().to_string()))
    } else {
        (s.to_string(), None)
    }
}

fn parse_id_icon_label(s: &str) -> (String, Option<String>, Option<String>) {
    let s = s.trim();
    // id(icon)[label]
    let icon_start = s.find('(');
    let label_start = s.find('[');
    let id_end = [icon_start, label_start]
        .iter()
        .flatten()
        .min()
        .copied()
        .unwrap_or(s.len());
    let id = s[..id_end].trim().to_string();
    let icon = if let (Some(o), Some(c)) = (s[id_end..].find('('), s[id_end..].find(')')) {
        // Upstream allows a quoted icon name (`("logos:aws-lambda")`); strip the
        // quotes so the fallback caption doesn't render a stray `"`.
        Some(
            s[id_end + o + 1..id_end + c]
                .trim()
                .trim_matches('"')
                .to_string(),
        )
    } else {
        None
    };
    let label = if let (Some(o), Some(c)) = (s.find('['), s.rfind(']')) {
        let raw = &s[o + 1..c];
        Some(raw.trim().trim_matches('"').to_string())
    } else {
        None
    };
    (id, icon, label)
}

fn parse_edge(line: &str, line_no: usize) -> Result<ArchEdge, ParseError> {
    // forms:
    //   id:S -- S:id
    //   id:S --> S:id            (with arrow on to)
    //   id:S <-- S:id            (with arrow on from)
    //   id:S -[title]- S:id      (titled edge)
    //   id{group}:S -- S:id      (group edge)
    let mut from_arrow = false;
    let mut to_arrow = false;
    let mut group = false;
    let mut label = None;

    // Connector is either `--` or the titled form `-[title]-` (upstream langium
    // Arrow `'--' | '-' title=ARCH_TITLE '-'`).
    let (left, right) = if let Some((l, title, r)) = split_titled_edge(line) {
        label = Some(title);
        (l, r)
    } else if let Some((l, r)) = line.split_once("--") {
        (l, r)
    } else {
        return Err(ParseError::malformed(
            line_no,
            format!("expected '--' edge: '{line}'"),
        ));
    };
    let mut left = left.trim();
    let mut right = right.trim();
    if let Some(s) = left.strip_suffix('<') {
        from_arrow = true;
        left = s.trim_end();
    }
    if let Some(s) = right.strip_prefix('>') {
        to_arrow = true;
        right = s.trim_start();
    }

    let (from, from_side) = parse_side(left)?;
    let (to_side, to) = parse_side_left(right)?;

    // group edge marker: id{group} — an endpoint referring to a group box
    // rather than a service. Strip the whole `{group}` suffix as a unit; a
    // char-by-char trim would eat the `}` first and leave `{group` dangling.
    let (from, from_group) = strip_group_marker(&from);
    let (to, to_group) = strip_group_marker(&to);
    group |= from_group || to_group;

    Ok(ArchEdge {
        from,
        from_side,
        from_arrow,
        to,
        to_side,
        to_arrow,
        group,
        label,
    })
}

/// Splits a titled edge `left -[title]- right` into its three parts. Returns
/// `None` when the line has no `-[…]-` connector.
fn split_titled_edge(line: &str) -> Option<(&str, String, &str)> {
    let open = line.find("-[")?;
    let close_rel = line[open + 2..].find("]-")?;
    let close = open + 2 + close_rel;
    let title = line[open + 2..close].trim().to_string();
    Some((&line[..open], title, &line[close + 2..]))
}

/// True for a `align row|column …` statement (v11.16+). Requires whitespace
/// after `align` so a service/edge id starting with `align` isn't captured.
fn is_align_stmt(line: &str) -> bool {
    match line.strip_prefix("align") {
        Some(rest) if rest.starts_with(char::is_whitespace) => {
            let kw = rest.trim_start();
            kw.starts_with("row") || kw.starts_with("column")
        }
        _ => false,
    }
}

/// Strips a trailing `{group}` endpoint marker, returning the bare id and
/// whether the marker was present.
fn strip_group_marker(id: &str) -> (String, bool) {
    match id.strip_suffix("{group}") {
        Some(base) => (base.trim_end().to_string(), true),
        None => (id.to_string(), false),
    }
}

fn parse_side(s: &str) -> Result<(String, ArchSide), ParseError> {
    let s = s.trim();
    let (id, side) = s.rsplit_once(':').unwrap_or((s, "R"));
    Ok((id.trim().to_string(), parse_side_char(side.trim())))
}

fn parse_side_left(s: &str) -> Result<(ArchSide, String), ParseError> {
    let s = s.trim();
    let (side, id) = s.split_once(':').unwrap_or(("L", s));
    Ok((parse_side_char(side.trim()), id.trim().to_string()))
}

fn parse_side_char(s: &str) -> ArchSide {
    match s {
        "T" | "t" | "top" => ArchSide::Top,
        "B" | "b" | "bottom" => ArchSide::Bottom,
        "L" | "l" | "left" => ArchSide::Left,
        _ => ArchSide::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let src = "architecture-beta\ngroup api(cloud)[API]\nservice db(database)[DB] in api\nservice disk1(disk)[Storage] in api\ndb:L -- R:disk1\n";
        let d = parse(src).unwrap();
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.services.len(), 2);
        assert_eq!(d.edges.len(), 1);
        assert_eq!(d.services[0].parent.as_deref(), Some("api"));
        assert_eq!(d.edges[0].from, "db");
        assert_eq!(d.edges[0].to, "disk1");
    }

    #[test]
    fn group_edge_ids_strip_marker() {
        // `{group}` marks an endpoint as a group box; the whole suffix must be
        // stripped as a unit (regression: trim ate `}` first, leaving `{group`).
        let src = "architecture-beta\ngroup left_disk(disk)[Left]\ngroup right_disk(disk)[Right]\nleft_disk{group}:R -- L:right_disk{group}\n";
        let d = parse(src).unwrap();
        assert_eq!(d.edges.len(), 1);
        let e = &d.edges[0];
        assert_eq!(e.from, "left_disk");
        assert_eq!(e.to, "right_disk");
        assert_eq!(e.from_side, ArchSide::Right);
        assert_eq!(e.to_side, ArchSide::Left);
        assert!(e.group);
    }

    #[test]
    fn edge_title() {
        // Upstream `-[title]-` connector carries an edge title (#184).
        let src = "architecture-beta\nservice db(database)[DB]\nservice server(server)[Srv]\ndb:R -[Queries]- L:server\n";
        let d = parse(src).unwrap();
        assert_eq!(d.edges.len(), 1);
        let e = &d.edges[0];
        assert_eq!(e.from, "db");
        assert_eq!(e.to, "server");
        assert_eq!(e.from_side, ArchSide::Right);
        assert_eq!(e.to_side, ArchSide::Left);
        assert_eq!(e.label.as_deref(), Some("Queries"));
    }

    #[test]
    fn edge_title_with_arrows() {
        let src =
            "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\na:R <-[link]-> L:b\n";
        let d = parse(src).unwrap();
        let e = &d.edges[0];
        assert!(e.from_arrow);
        assert!(e.to_arrow);
        assert_eq!(e.label.as_deref(), Some("link"));
    }

    #[test]
    fn align_statement_consumed() {
        // `align row|column id id…` (v11.16+) is consumed, not a hard error (#184).
        let src = "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\nalign row a b\n";
        let d = parse(src).unwrap();
        assert_eq!(d.services.len(), 2);
        assert_eq!(d.edges.len(), 0);
    }

    #[test]
    fn quoted_icon_name_strips_quotes() {
        // A quoted iconify name must not leak the quote into the fallback caption (#184).
        let src = "architecture-beta\nservice lambda(\"logos:aws-lambda\")[Lambda]\n";
        let d = parse(src).unwrap();
        assert_eq!(d.services[0].icon.as_deref(), Some("logos:aws-lambda"));
    }
}
