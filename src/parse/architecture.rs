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
                return Err(ParseError::Syntax {
                    message: "expected 'architecture-beta' header".into(),
                    line: line_no,
                });
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
        Some(s[id_end + o + 1..id_end + c].to_string())
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
    //   id:S --> S:id      (with arrow on to)
    //   id:S <-- S:id      (with arrow on from)
    //   id{group}:S -- S:id   (group edge)
    let mut from_arrow = false;
    let mut to_arrow = false;
    let mut group = false;

    let pat = if line.contains("--") {
        "--"
    } else {
        return Err(ParseError::Syntax {
            message: format!("expected '--' edge: '{line}'"),
            line: line_no,
        });
    };
    let (left, right) = line.split_once(pat).unwrap();
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

    // group edge marker: id{group}
    if from.contains('{') || to.contains('{') {
        group = true;
    }

    Ok(ArchEdge {
        from: from
            .trim_end_matches('{')
            .trim_end_matches('}')
            .trim_end_matches("{group}")
            .to_string(),
        from_side,
        from_arrow,
        to: to
            .trim_end_matches('{')
            .trim_end_matches('}')
            .trim_end_matches("{group}")
            .to_string(),
        to_side,
        to_arrow,
        label: None,
        group,
    })
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
}
