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
                .ok_or_else(|| ParseError::Syntax {
                    message: "expected 'xychart-beta' header".into(),
                    line: line_no,
                })?;
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
            d.series.push(XySeries {
                kind: XySeriesKind::Bar,
                values: parse_value_list(rest, line_no)?,
            });
        } else if let Some(rest) = line.strip_prefix("line") {
            d.series.push(XySeries {
                kind: XySeriesKind::Line,
                values: parse_value_list(rest, line_no)?,
            });
        } else {
            return Err(ParseError::Syntax {
                message: format!("unknown xychart line: '{line}'"),
                line: line_no,
            });
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_axis(rest: &str, line_no: usize) -> Result<XyAxis, ParseError> {
    let rest = rest.trim();
    let (title, body) = if rest.starts_with('"') {
        let end = rest[1..].find('"').ok_or_else(|| ParseError::Syntax {
            message: "unterminated string in axis".into(),
            line: line_no,
        })?;
        (Some(rest[1..1 + end].to_string()), rest[2 + end..].trim())
    } else {
        (None, rest)
    };
    let kind = if body.contains("-->") {
        let (lo, hi) = body.split_once("-->").unwrap();
        let min: f64 = lo.trim().parse().map_err(|_| ParseError::Syntax {
            message: format!("invalid axis min: '{}'", lo.trim()),
            line: line_no,
        })?;
        let max: f64 = hi.trim().parse().map_err(|_| ParseError::Syntax {
            message: format!("invalid axis max: '{}'", hi.trim()),
            line: line_no,
        })?;
        XyAxisKind::Range { min, max }
    } else if body.starts_with('[') {
        let inner = body.trim_start_matches('[').trim_end_matches(']');
        let cats: Vec<String> = inner
            .split(',')
            .map(|s| unquote(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .collect();
        XyAxisKind::Categories(cats)
    } else if body.is_empty() {
        XyAxisKind::Categories(Vec::new())
    } else {
        return Err(ParseError::Syntax {
            message: format!("invalid axis body: '{body}'"),
            line: line_no,
        });
    };
    Ok(XyAxis { title, kind })
}

fn parse_value_list(rest: &str, line_no: usize) -> Result<Vec<f64>, ParseError> {
    let body = rest.trim().trim_start_matches('[').trim_end_matches(']');
    let mut out = Vec::new();
    for s in body.split(',') {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        let v: f64 = s.parse().map_err(|_| ParseError::Syntax {
            message: format!("invalid value: '{s}'"),
            line: line_no,
        })?;
        out.push(v);
    }
    Ok(out)
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
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
