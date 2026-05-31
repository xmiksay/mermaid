//! radar-beta parser.
//!
//! Grammar:
//!
//! ```text
//! radar-beta
//!     title "Skills"
//!     axis A["Power"], B["Speed"], C["Endurance"]
//!     curve a["Athlete A"]{85, 90, 80}
//!     curve b["Athlete B"]{75, 85, 95}
//!     max 100
//! ```

use super::ast::{RadarAxis, RadarCurve, RadarDiagram};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<RadarDiagram, ParseError> {
    let mut d = RadarDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "radar-beta" && line != "radar" {
                return Err(ParseError::Syntax {
                    message: "expected 'radar-beta' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(unquote(rest.trim()).to_string());
        } else if let Some(rest) = line.strip_prefix("axis") {
            d.axes = parse_axis_list(rest, line_no)?;
        } else if let Some(rest) = line.strip_prefix("curve") {
            d.curves.push(parse_curve(rest, line_no)?);
        } else if let Some(rest) = line.strip_prefix("max") {
            d.max = Some(rest.trim().parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid max: '{}'", rest.trim()),
                line: line_no,
            })?);
        } else {
            return Err(ParseError::Syntax {
                message: format!("unknown radar line: '{line}'"),
                line: line_no,
            });
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_axis_list(s: &str, line_no: usize) -> Result<Vec<RadarAxis>, ParseError> {
    let mut out = Vec::new();
    for item in split_top(s, ',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (id, label) = if let Some(open) = item.find('[') {
            let id = item[..open].trim().to_string();
            let close = item.rfind(']').ok_or_else(|| ParseError::Syntax {
                message: format!("missing ']' in axis '{item}'"),
                line: line_no,
            })?;
            let label = unquote(item[open + 1..close].trim()).to_string();
            (id, label)
        } else {
            (item.to_string(), item.to_string())
        };
        out.push(RadarAxis { id, label });
    }
    Ok(out)
}

fn parse_curve(s: &str, line_no: usize) -> Result<RadarCurve, ParseError> {
    let s = s.trim();
    // form: id["label"]{v1, v2, ...} or id{v1, v2, ...}
    let brace = s.find('{').ok_or_else(|| ParseError::Syntax {
        message: format!("expected '{{values}}' in curve '{s}'"),
        line: line_no,
    })?;
    let head = &s[..brace];
    let body = s[brace + 1..].trim_end_matches('}').trim();
    let (id, label) = if let Some(open) = head.find('[') {
        let id = head[..open].trim().to_string();
        let close = head.rfind(']').ok_or_else(|| ParseError::Syntax {
            message: format!("missing ']' in curve '{s}'"),
            line: line_no,
        })?;
        let label = unquote(head[open + 1..close].trim()).to_string();
        (id, label)
    } else {
        (head.trim().to_string(), head.trim().to_string())
    };
    let values: Result<Vec<f64>, _> = body.split(',').map(|v| v.trim().parse::<f64>()).collect();
    let values = values.map_err(|_| ParseError::Syntax {
        message: format!("invalid value list: '{body}'"),
        line: line_no,
    })?;
    Ok(RadarCurve { id, label, values })
}

fn split_top(s: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '[' | '{' | '(' => {
                depth += 1;
                cur.push(c);
            }
            ']' | '}' | ')' => {
                depth -= 1;
                cur.push(c);
            }
            d if d == delim && depth == 0 => {
                out.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
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
    fn minimal() {
        let d =
            parse("radar-beta\naxis A[\"Power\"], B[\"Speed\"]\ncurve a[\"X\"]{1, 2}\n").unwrap();
        assert_eq!(d.axes.len(), 2);
        assert_eq!(d.axes[0].label, "Power");
        assert_eq!(d.curves[0].values, vec![1.0, 2.0]);
    }

    #[test]
    fn with_max() {
        let d = parse("radar-beta\naxis A, B\ncurve a{1, 2}\nmax 10\n").unwrap();
        assert_eq!(d.max, Some(10.0));
    }
}
