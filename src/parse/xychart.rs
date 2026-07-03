//! xychart-beta parser.
//!
//! Grammar:
//!
//! ```text
//! xychart-beta [horizontal]
//!     title "Title"
//!     x-axis "Label" [a, b, c]            // categorical
//!     x-axis 0 --> 12                     // numeric range
//!     y-axis "Label" 0 --> 100
//!     bar [1, 2, 3]
//!     line [1, 2, 3]
//! ```

use super::ast::{XyAxis, XyAxisKind, XyChartDiagram, XySeries, XySeriesKind};
use super::token::{split_unquoted, unquote};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<XyChartDiagram, ParseError> {
    let mut d = XyChartDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            let rest = line
                .strip_prefix("xychart-beta")
                .or_else(|| line.strip_prefix("xychart"))
                .ok_or_else(|| ParseError::header(line_no, "expected 'xychart-beta' header"))?;
            d.horizontal = rest.trim() == "horizontal";
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(unquote(rest.trim()).to_string());
        } else if let Some(rest) = line.strip_prefix("x-axis") {
            d.x_axis = Some(parse_axis(rest, line_no)?);
        } else if let Some(rest) = line.strip_prefix("y-axis") {
            d.y_axis = Some(parse_axis(rest, line_no)?);
        } else if let Some(rest) = line.strip_prefix("bar") {
            let (title, values) = parse_series(rest, line_no)?;
            d.series.push(XySeries {
                kind: XySeriesKind::Bar,
                title,
                values,
            });
        } else if let Some(rest) = line.strip_prefix("line") {
            let (title, values) = parse_series(rest, line_no)?;
            d.series.push(XySeries {
                kind: XySeriesKind::Line,
                title,
                values,
            });
        } else {
            return Err(ParseError::unknown(
                line_no,
                format!("unknown xychart line: '{line}'"),
            ));
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_axis(rest: &str, line_no: usize) -> Result<XyAxis, ParseError> {
    let rest = rest.trim();
    let (title, body) = if let Some(after) = rest.strip_prefix('"') {
        let end = after
            .find('"')
            .ok_or_else(|| ParseError::unclosed(line_no, "unterminated string in axis"))?;
        (Some(after[..end].to_string()), after[end + 1..].trim())
    } else {
        (None, rest)
    };
    let kind = if body.contains("-->") {
        let (lo, hi) = body.split_once("-->").unwrap();
        let min: f64 = lo.trim().parse().map_err(|_| {
            ParseError::number(line_no, format!("invalid axis min: '{}'", lo.trim()))
        })?;
        let max: f64 = hi.trim().parse().map_err(|_| {
            ParseError::number(line_no, format!("invalid axis max: '{}'", hi.trim()))
        })?;
        XyAxisKind::Range { min, max }
    } else if body.starts_with('[') {
        let inner = body.trim_start_matches('[').trim_end_matches(']');
        // Quote-aware so a category containing a comma (`"a,b"`) stays one cell.
        let cats: Vec<String> = split_unquoted(inner, ',')
            .iter()
            .map(|s| unquote(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .collect();
        XyAxisKind::Categories(cats)
    } else if body.is_empty() {
        XyAxisKind::Categories(Vec::new())
    } else {
        return Err(ParseError::malformed(
            line_no,
            format!("invalid axis body: '{body}'"),
        ));
    };
    Ok(XyAxis { title, kind })
}

/// Parse a `bar`/`line` body: an optional quoted series title followed by the
/// `[..]` value list — `bar "Revenue" [10, 20]` or `line [1, 2]`.
fn parse_series(rest: &str, line_no: usize) -> Result<(Option<String>, Vec<f64>), ParseError> {
    let rest = rest.trim();
    let (title, list) = if let Some(after) = rest.strip_prefix('"') {
        let end = after
            .find('"')
            .ok_or_else(|| ParseError::unclosed(line_no, "unterminated string in series title"))?;
        (Some(after[..end].to_string()), after[end + 1..].trim())
    } else {
        (None, rest)
    };
    Ok((title, parse_value_list(list, line_no)?))
}

fn parse_value_list(rest: &str, line_no: usize) -> Result<Vec<f64>, ParseError> {
    let body = rest.trim().trim_start_matches('[').trim_end_matches(']');
    let mut out = Vec::new();
    for s in body.split(',') {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        let v: f64 = s
            .parse()
            .map_err(|_| ParseError::number(line_no, format!("invalid value: '{s}'")))?;
        out.push(v);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bar_and_line() {
        let d = parse(
            "xychart-beta\ntitle \"Sales\"\nx-axis [jan, feb, mar]\ny-axis 0 --> 100\nbar [10, 20, 30]\nline [5, 15, 25]\n",
        ).unwrap();
        assert_eq!(d.title.as_deref(), Some("Sales"));
        match &d.x_axis.as_ref().unwrap().kind {
            XyAxisKind::Categories(c) => assert_eq!(c.len(), 3),
            _ => panic!(),
        }
        assert_eq!(d.series.len(), 2);
        assert_eq!(d.series[0].values, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn parses_quoted_series_title() {
        let d =
            parse("xychart-beta\nbar \"Revenue\" [10, 20, 30]\nline \"Trend\" [5, 15]\n").unwrap();
        assert_eq!(d.series.len(), 2);
        assert_eq!(d.series[0].title.as_deref(), Some("Revenue"));
        assert_eq!(d.series[0].values, vec![10.0, 20.0, 30.0]);
        assert_eq!(d.series[1].title.as_deref(), Some("Trend"));
        assert_eq!(d.series[1].kind, XySeriesKind::Line);
    }

    #[test]
    fn category_with_quoted_comma_stays_one_cell() {
        let d = parse("xychart-beta\nx-axis [jan, \"a, b\", mar]\nbar [1, 2, 3]\n").unwrap();
        match &d.x_axis.as_ref().unwrap().kind {
            XyAxisKind::Categories(c) => {
                assert_eq!(
                    c,
                    &vec!["jan".to_string(), "a, b".to_string(), "mar".to_string()]
                );
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parses_numeric_x_axis() {
        let d = parse("xychart-beta\nx-axis 0 --> 10\nbar [1, 2]\n").unwrap();
        match d.x_axis.unwrap().kind {
            XyAxisKind::Range { min, max } => {
                assert_eq!(min, 0.0);
                assert_eq!(max, 10.0);
            }
            _ => panic!(),
        }
    }
}
