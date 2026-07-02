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
//!     <Point label>: [x, y, r]
//!     <Point label>:::<class>: [x, y] radius: 8, color: #ff0000
//!     classDef <class> color: #ff0000, stroke-width: 2px
//! ```

use super::ast::{QuadrantDiagram, QuadrantPoint, QuadrantStyle};
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
        } else if let Some(rest) = line.strip_prefix("classDef") {
            let (name, style) = rest
                .trim()
                .split_once(' ')
                .ok_or_else(|| ParseError::Syntax {
                    message: format!("expected 'classDef <name> <style>': '{line}'"),
                    line: line_no,
                })?;
            d.classes
                .insert(name.trim().to_string(), parse_style(style));
        } else {
            d.points.push(parse_point(line, line_no)?);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

/// `label: [x, y]`, `label: [x, y, r]`, or
/// `label:::class: [x, y] radius: 8, color: #ff0000`.
fn parse_point(line: &str, line_no: usize) -> Result<QuadrantPoint, ParseError> {
    let open = line.find('[').ok_or_else(|| ParseError::Syntax {
        message: format!("expected 'label: [x, y]': '{line}'"),
        line: line_no,
    })?;
    let close = line[open..]
        .find(']')
        .map(|i| open + i)
        .ok_or_else(|| ParseError::Syntax {
            message: format!("unterminated coordinate list: '{line}'"),
            line: line_no,
        })?;

    // Prefix is "<label>:" or "<label>:::<class>:"; the trailing ':' separates it
    // from the coordinate list.
    let prefix = line[..open].trim().trim_end_matches(':').trim();
    let (label, class_name) = match prefix.split_once(":::") {
        Some((l, c)) => (l.trim(), Some(c.trim().to_string())),
        None => (prefix, None),
    };

    let coords = &line[open + 1..close];
    let mut parts = coords.split(',');
    let x = parse_coord(parts.next(), "x", coords, line_no)?;
    let y = parse_coord(parts.next(), "y", coords, line_no)?;
    let radius = match parts.next().map(str::trim) {
        Some(r) if !r.is_empty() => Some(r.parse().map_err(|_| ParseError::Syntax {
            message: format!("invalid radius in '{coords}'"),
            line: line_no,
        })?),
        _ => None,
    };

    // Inline styling after the coordinate list, e.g. "radius: 8, color: #f00".
    let style = parse_style(line[close + 1..].trim());
    Ok(QuadrantPoint {
        label: label.to_string(),
        x,
        y,
        radius: style.radius.or(radius),
        color: style.color,
        stroke_color: style.stroke_color,
        stroke_width: style.stroke_width,
        class_name,
    })
}

fn parse_coord(
    part: Option<&str>,
    which: &str,
    coords: &str,
    line_no: usize,
) -> Result<f64, ParseError> {
    part.unwrap_or("")
        .trim()
        .parse()
        .map_err(|_| ParseError::Syntax {
            message: format!("invalid {which} in '{coords}'"),
            line: line_no,
        })
}

/// Comma-separated `key: value` styling attributes.
fn parse_style(s: &str) -> QuadrantStyle {
    let mut style = QuadrantStyle::default();
    for attr in s.split(',') {
        let Some((k, v)) = attr.split_once(':') else {
            continue;
        };
        let v = v.trim().to_string();
        match k.trim() {
            "radius" => style.radius = v.parse().ok(),
            "color" => style.color = Some(v),
            "stroke-color" => style.stroke_color = Some(v),
            "stroke-width" => style.stroke_width = Some(v),
            _ => {}
        }
    }
    style
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
        assert_eq!(d.points[0].radius, None);
    }

    #[test]
    fn third_value_is_radius() {
        let d = parse("quadrantChart\nA: [0.3, 0.6, 8]\n").unwrap();
        let p = &d.points[0];
        assert_eq!(p.x, 0.3);
        assert_eq!(p.y, 0.6);
        assert_eq!(p.radius, Some(8.0));
    }

    #[test]
    fn inline_styling() {
        let d = parse(
            "quadrantChart\nA: [0.3, 0.6] radius: 10, color: #ff0000, stroke-color: #00ff00, stroke-width: 5px\n",
        )
        .unwrap();
        let p = &d.points[0];
        assert_eq!(p.radius, Some(10.0));
        assert_eq!(p.color.as_deref(), Some("#ff0000"));
        assert_eq!(p.stroke_color.as_deref(), Some("#00ff00"));
        assert_eq!(p.stroke_width.as_deref(), Some("5px"));
    }

    #[test]
    fn inline_radius_overrides_array_radius() {
        let d = parse("quadrantChart\nA: [0.3, 0.6, 8] radius: 20\n").unwrap();
        assert_eq!(d.points[0].radius, Some(20.0));
    }

    #[test]
    fn class_ref_and_classdef() {
        let d =
            parse("quadrantChart\nclassDef hot color: #ff0000, radius: 15\nA:::hot: [0.3, 0.6]\n")
                .unwrap();
        let p = &d.points[0];
        assert_eq!(p.label, "A");
        assert_eq!(p.class_name.as_deref(), Some("hot"));
        let style = d.classes.get("hot").unwrap();
        assert_eq!(style.color.as_deref(), Some("#ff0000"));
        assert_eq!(style.radius, Some(15.0));
    }
}
