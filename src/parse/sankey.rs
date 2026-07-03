//! Sankey-beta parser. CSV body:
//!
//! ```text
//! sankey-beta
//! source,target,value
//! a,b,5
//! ```
//!
//! The optional first body line `source,target,value` is recognised as a
//! header and skipped. Quotes around fields are stripped.

use super::ast::{SankeyDiagram, SankeyLink};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<SankeyDiagram, ParseError> {
    let mut d = SankeyDiagram::default();
    let mut header_seen = false;
    let mut csv_header_skipped = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "sankey-beta" && line != "sankey" {
                return Err(ParseError::header(line_no, "expected 'sankey-beta' header"));
            }
            header_seen = true;
            continue;
        }

        if !csv_header_skipped {
            csv_header_skipped = true;
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("source,target,value") {
                continue;
            }
        }

        let fields: Vec<String> = split_csv(line);
        if fields.len() < 3 {
            return Err(ParseError::malformed(
                line_no,
                format!("expected 'source,target,value': '{line}'"),
            ));
        }
        let value: f64 = fields[2]
            .parse()
            .map_err(|_| ParseError::number(line_no, format!("invalid value: '{}'", fields[2])))?;
        d.links.push(SankeyLink {
            source: fields[0].clone(),
            target: fields[1].clone(),
            value,
        });
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_q && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_q = !in_q,
            ',' if !in_q => {
                out.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let d = parse("sankey-beta\nA,B,5\n").unwrap();
        assert_eq!(d.links.len(), 1);
        assert_eq!(d.links[0].source, "A");
        assert_eq!(d.links[0].value, 5.0);
    }

    #[test]
    fn skips_csv_header() {
        let d = parse("sankey-beta\nsource,target,value\nA,B,5\nC,D,7\n").unwrap();
        assert_eq!(d.links.len(), 2);
    }

    #[test]
    fn quoted_field() {
        let d = parse("sankey-beta\n\"A,1\",B,3\n").unwrap();
        assert_eq!(d.links[0].source, "A,1");
    }

    #[test]
    fn config_link_color_and_node_alignment() {
        let src = "---\nconfig:\n  sankey:\n    linkColor: gradient\n    nodeAlignment: right\n---\nsankey-beta\nA,B,5\n";
        let crate::parse::ast::Diagram::Sankey(d) = crate::parse::parse(src).unwrap() else {
            panic!("expected sankey");
        };
        assert_eq!(d.link_color.as_deref(), Some("gradient"));
        assert_eq!(d.node_alignment.as_deref(), Some("right"));
    }

    #[test]
    fn config_show_values_prefix_suffix_and_geometry() {
        let src = "---\nconfig:\n  sankey:\n    showValues: false\n    prefix: \"$\"\n    suffix: \" USD\"\n    width: 600\n    height: 400\n    nodeWidth: 12\n    nodePadding: 8\n---\nsankey-beta\nA,B,5\n";
        let crate::parse::ast::Diagram::Sankey(d) = crate::parse::parse(src).unwrap() else {
            panic!("expected sankey");
        };
        assert_eq!(d.show_values, Some(false));
        assert_eq!(d.prefix.as_deref(), Some("$"));
        assert_eq!(d.suffix.as_deref(), Some(" USD"));
        assert_eq!(d.width, Some(600.0));
        assert_eq!(d.height, Some(400.0));
        assert_eq!(d.node_width, Some(12.0));
        assert_eq!(d.node_padding, Some(8.0));
    }
}
