//! packet-beta parser.
//!
//! Grammar:
//!
//! ```text
//! packet-beta
//!     title TCP header
//!     0-15: "Source Port"
//!     16-31: "Destination Port"
//!     32: "Single bit"
//! ```

use super::ast::{PacketDiagram, PacketField};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<PacketDiagram, ParseError> {
    let mut d = PacketDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "packet-beta" && line != "packet" {
                return Err(ParseError::Syntax {
                    message: "expected 'packet-beta' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
            continue;
        }

        let (range, label) = line.split_once(':').ok_or_else(|| ParseError::Syntax {
            message: format!("expected '<range>: \"label\"': '{line}'"),
            line: line_no,
        })?;
        let (start, end) = if let Some((s, e)) = range.split_once('-') {
            let s: u32 = s.trim().parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid start '{}'", s.trim()),
                line: line_no,
            })?;
            let e: u32 = e.trim().parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid end '{}'", e.trim()),
                line: line_no,
            })?;
            (s, e)
        } else {
            let s: u32 = range.trim().parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid bit '{}'", range.trim()),
                line: line_no,
            })?;
            (s, s)
        };
        let label = label.trim().trim_matches('"').to_string();
        d.fields.push(PacketField { start, end, label });
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let d = parse("packet-beta\n0-15: \"Source\"\n16-31: \"Dest\"\n").unwrap();
        assert_eq!(d.fields.len(), 2);
        assert_eq!(d.fields[0].start, 0);
        assert_eq!(d.fields[0].end, 15);
        assert_eq!(d.fields[0].label, "Source");
    }

    #[test]
    fn single_bit() {
        let d = parse("packet-beta\n0: \"Flag\"\n").unwrap();
        assert_eq!(d.fields[0].start, 0);
        assert_eq!(d.fields[0].end, 0);
    }
}
