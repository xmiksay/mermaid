//! xychart-beta parser.
//!
//! Grammar:
//!
//! ```text
//! xychart-beta [horizontal]
//!     title "Title"
//!     x-axis "Label" [a, b, c]            // categorical
//!     x-axis time 0 --> 12                // numeric range, bare-word title
//!     y-axis revenue                      // bare-word title only
//!     bar [1, 2, 3]
//!     line [1.5 "label", 2, 3]            // optional per-point label
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
            let (title, values, labels) = parse_series(rest, line_no)?;
            d.series.push(XySeries {
                kind: XySeriesKind::Bar,
                title,
                values,
                labels,
            });
        } else if let Some(rest) = line.strip_prefix("line") {
            let (title, values, labels) = parse_series(rest, line_no)?;
            d.series.push(XySeries {
                kind: XySeriesKind::Line,
                title,
                values,
                labels,
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
    // Upstream `text: alphaNum | STR | MD_STR`: the title may be quoted or a
    // bare word run, and precedes an optional `[..]` band or `min --> max`
    // range — `x-axis time 0 --> 10`, `y-axis revenue`, `x-axis "Label" [a, b]`.
    let (title, body) = if let Some(after) = rest.strip_prefix('"') {
        let end = after
            .find('"')
            .ok_or_else(|| ParseError::unclosed(line_no, "unterminated string in axis"))?;
        (
            Some(after[..end].to_string()),
            after[end + 1..].trim().to_string(),
        )
    } else if let Some(pos) = rest.find("-->") {
        // The word just before `-->` is the range min; earlier words are title.
        match rest[..pos].trim().rsplit_once(char::is_whitespace) {
            Some((head, min)) => (title_opt(head), format!("{min} {}", &rest[pos..])),
            None => (None, rest.to_string()),
        }
    } else if let Some(pos) = rest.find('[') {
        (title_opt(&rest[..pos]), rest[pos..].to_string())
    } else {
        // No band, no range: the whole line is a bare axis title.
        (title_opt(rest), String::new())
    };
    let body = body.trim();
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

/// Trim a bare-word title candidate, returning `None` when it is empty.
fn title_opt(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Parse a `bar`/`line` body: an optional quoted series title followed by the
/// `[..]` value list — `bar "Revenue" [10, 20]` or `line [1, 2]`. Each list
/// entry may carry an optional per-point label (`line [1.5 "label", 2.3]`).
type Series = (Option<String>, Vec<f64>, Vec<Option<String>>);

fn parse_series(rest: &str, line_no: usize) -> Result<Series, ParseError> {
    let rest = rest.trim();
    let (title, list) = if let Some(after) = rest.strip_prefix('"') {
        let end = after
            .find('"')
            .ok_or_else(|| ParseError::unclosed(line_no, "unterminated string in series title"))?;
        (Some(after[..end].to_string()), after[end + 1..].trim())
    } else {
        (None, rest)
    };
    let (values, labels) = parse_value_list(list, line_no)?;
    Ok((title, values, labels))
}

fn parse_value_list(
    rest: &str,
    line_no: usize,
) -> Result<(Vec<f64>, Vec<Option<String>>), ParseError> {
    let body = rest.trim().trim_start_matches('[').trim_end_matches(']');
    let mut values = Vec::new();
    let mut labels = Vec::new();
    // Quote-aware so a per-point label containing a comma survives the split.
    for cell in split_unquoted(body, ',') {
        let cell = cell.trim();
        if cell.is_empty() {
            continue;
        }
        // Upstream `dataPoint: NUMBER_WITH_DECIMAL STR`: a value with an
        // optional trailing quoted label.
        let (num, label) = match cell.find('"') {
            Some(q) => {
                let after = &cell[q + 1..];
                let end = after
                    .find('"')
                    .ok_or_else(|| ParseError::unclosed(line_no, "unterminated point label"))?;
                (cell[..q].trim(), Some(after[..end].to_string()))
            }
            None => (cell, None),
        };
        let v: f64 = num
            .parse()
            .map_err(|_| ParseError::number(line_no, format!("invalid value: '{num}'")))?;
        values.push(v);
        labels.push(label);
    }
    Ok((values, labels))
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
    fn parses_unquoted_axis_title_with_range() {
        let d = parse("xychart-beta\nx-axis time 0 --> 10\nbar [1, 2]\n").unwrap();
        let ax = d.x_axis.unwrap();
        assert_eq!(ax.title.as_deref(), Some("time"));
        match ax.kind {
            XyAxisKind::Range { min, max } => {
                assert_eq!(min, 0.0);
                assert_eq!(max, 10.0);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parses_bare_axis_title() {
        let d = parse("xychart-beta\ny-axis revenue\nbar [1, 2]\n").unwrap();
        let ax = d.y_axis.unwrap();
        assert_eq!(ax.title.as_deref(), Some("revenue"));
        assert_eq!(ax.kind, XyAxisKind::Categories(Vec::new()));
    }

    #[test]
    fn parses_unquoted_axis_title_with_band() {
        let d = parse("xychart-beta\nx-axis month [jan, feb]\nbar [1, 2]\n").unwrap();
        let ax = d.x_axis.unwrap();
        assert_eq!(ax.title.as_deref(), Some("month"));
        match ax.kind {
            XyAxisKind::Categories(c) => assert_eq!(c, vec!["jan", "feb"]),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_per_point_labels() {
        let d = parse("xychart-beta\nline [1.5 \"low\", 2.3, 4.0 \"high\"]\n").unwrap();
        let s = &d.series[0];
        assert_eq!(s.values, vec![1.5, 2.3, 4.0]);
        assert_eq!(
            s.labels,
            vec![Some("low".to_string()), None, Some("high".to_string())]
        );
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
