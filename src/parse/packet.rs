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
//!     +16: "Relative (next 16 bits)"
//! ```
//!
//! A leading `+` marks a *relative* field: `+N` occupies the next `N` bits
//! starting right after the previous field (upstream packet v11.7+).

use super::ast::{PacketDiagram, PacketField};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<PacketDiagram, ParseError> {
    let mut d = PacketDiagram::default();
    let mut header_seen = false;
    // Next free bit position, for resolving relative `+N` fields.
    let mut cursor: u32 = 0;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "packet-beta" && line != "packet" {
                return Err(ParseError::header(line_no, "expected 'packet-beta' header"));
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
            continue;
        }

        let (range, label) = line.split_once(':').ok_or_else(|| {
            ParseError::malformed(line_no, format!("expected '<range>: \"label\"': '{line}'"))
        })?;
        let range = range.trim();
        let (start, end) = if let Some(count) = range.strip_prefix('+') {
            // Relative field: `+N` occupies the next N bits after the cursor.
            let n: u32 = count.trim().parse().map_err(|_| {
                ParseError::number(
                    line_no,
                    format!("invalid relative width '{}'", count.trim()),
                )
            })?;
            if n == 0 {
                return Err(ParseError::number(
                    line_no,
                    "relative field width must be at least 1",
                ));
            }
            (cursor, cursor + n - 1)
        } else if let Some((s, e)) = range.split_once('-') {
            let s: u32 = s.trim().parse().map_err(|_| {
                ParseError::number(line_no, format!("invalid start '{}'", s.trim()))
            })?;
            let e: u32 = e
                .trim()
                .parse()
                .map_err(|_| ParseError::number(line_no, format!("invalid end '{}'", e.trim())))?;
            (s, e)
        } else {
            let s: u32 = range
                .parse()
                .map_err(|_| ParseError::number(line_no, format!("invalid bit '{range}'")))?;
            (s, s)
        };
        let label = label.trim().trim_matches('"').to_string();
        cursor = end + 1;
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

    #[test]
    fn relative_fields() {
        let d = parse("packet\n+16: \"Source Port\"\n+16: \"Destination Port\"\n").unwrap();
        assert_eq!(d.fields.len(), 2);
        assert_eq!((d.fields[0].start, d.fields[0].end), (0, 15));
        assert_eq!((d.fields[1].start, d.fields[1].end), (16, 31));
    }

    #[test]
    fn relative_after_absolute() {
        let d = parse("packet\n0-15: \"A\"\n+16: \"B\"\n").unwrap();
        assert_eq!((d.fields[1].start, d.fields[1].end), (16, 31));
    }

    #[test]
    fn relative_zero_width_is_error() {
        assert!(parse("packet\n+0: \"nope\"\n").is_err());
    }
}
