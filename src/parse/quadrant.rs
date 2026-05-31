//! Quadrant chart parser.
//!
//! Grammar:
//!
//! ```text
//! quadrantChart
//!     title <text>
//!     x-axis <left> --> <right>
//!     y-axis <bottom> --> <top>
//!     quadrant-1 <text>
//!     quadrant-2 <text>
//!     quadrant-3 <text>
//!     quadrant-4 <text>
//!     <Point label>: [x, y]
//! ```

use super::ast::{QuadrantDiagram, QuadrantPoint};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<QuadrantDiagram, ParseError> {
    let mut d = QuadrantDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "quadrantChart" {
                return Err(ParseError::Syntax {
                    message: "expected 'quadrantChart' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("x-axis") {
            let (l, r) = split_axis(rest);
            d.x_axis_left = l;
            d.x_axis_right = r;
        } else if let Some(rest) = line.strip_prefix("y-axis") {
            let (b, t) = split_axis(rest);
            d.y_axis_bottom = b;
            d.y_axis_top = t;
        } else if let Some(rest) = line.strip_prefix("quadrant-1") {
            d.q1 = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("quadrant-2") {
            d.q2 = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("quadrant-3") {
            d.q3 = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("quadrant-4") {
            d.q4 = Some(rest.trim().to_string());
        } else {
            // Point: "label: [x, y]"
            let (label, rest) = line.split_once(':').ok_or_else(|| ParseError::Syntax {
                message: format!("expected 'label: [x, y]': '{line}'"),
                line: line_no,
            })?;
            let coords = rest
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim();
            let mut parts = coords.split(',');
            let xs = parts.next().unwrap_or("").trim();
            let ys = parts.next().unwrap_or("").trim();
            let x: f64 = xs.parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid x in '{coords}'"),
                line: line_no,
            })?;
            let y: f64 = ys.parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid y in '{coords}'"),
                line: line_no,
            })?;
            d.points.push(QuadrantPoint {
                label: label.trim().to_string(),
                x,
                y,
            });
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn split_axis(s: &str) -> (Option<String>, Option<String>) {
    let s = s.trim();
    if let Some((l, r)) = s.split_once("-->") {
        (Some(l.trim().to_string()), Some(r.trim().to_string()))
    } else if !s.is_empty() {
        (Some(s.to_string()), None)
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let d = parse(
            "quadrantChart\ntitle T\nx-axis low --> high\ny-axis bottom --> top\nquadrant-1 Q1\nA: [0.3, 0.6]\n",
        )
        .unwrap();
        assert_eq!(d.title.as_deref(), Some("T"));
        assert_eq!(d.x_axis_left.as_deref(), Some("low"));
        assert_eq!(d.x_axis_right.as_deref(), Some("high"));
        assert_eq!(d.q1.as_deref(), Some("Q1"));
        assert_eq!(d.points.len(), 1);
        assert_eq!(d.points[0].label, "A");
        assert_eq!(d.points[0].x, 0.3);
    }
}
