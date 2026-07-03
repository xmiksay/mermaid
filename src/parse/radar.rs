//! radar-beta parser.
//!
//! Grammar:
//!
//! ```text
//! radar-beta
//!     title "Skills"
//!     axis A["Power"], B["Speed"], C["Endurance"]
//!     curve a["Athlete A"]{85, 90, 80}
//!     curve b["Athlete B"]{ Power: 75, Endurance: 95, Speed: 85 }
//!     min 0
//!     max 100
//!     ticks 5
//!     graticule circle
//!     showLegend true
//! ```
//!
//! `axis` statements accumulate (several `axis` lines append rather than
//! overwrite). Curve bodies are either a positional value list or `key: value`
//! pairs matched to axes by id/label (order-independent).

use std::collections::HashMap;

use super::ast::{RadarAxis, RadarCurve, RadarDiagram, RadarGraticule};
use super::token::unquote;
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
            d.axes.extend(parse_axis_list(rest, line_no)?);
        } else if let Some(rest) = line.strip_prefix("curve") {
            let curve = parse_curve(rest, &d.axes, line_no)?;
            d.curves.push(curve);
        } else if let Some(rest) = line.strip_prefix("showLegend") {
            d.show_legend = Some(parse_bool(rest.trim()));
        } else if let Some(rest) = line.strip_prefix("min") {
            d.min = Some(parse_num(rest.trim(), "min", line_no)?);
        } else if let Some(rest) = line.strip_prefix("max") {
            d.max = Some(parse_num(rest.trim(), "max", line_no)?);
        } else if let Some(rest) = line.strip_prefix("ticks") {
            d.ticks = Some(rest.trim().parse().map_err(|_| ParseError::Syntax {
                message: format!("invalid ticks: '{}'", rest.trim()),
                line: line_no,
            })?);
        } else if let Some(rest) = line.strip_prefix("graticule") {
            d.graticule = match rest.trim() {
                "polygon" => RadarGraticule::Polygon,
                "circle" | "" => RadarGraticule::Circle,
                other => {
                    return Err(ParseError::Syntax {
                        message: format!("invalid graticule: '{other}'"),
                        line: line_no,
                    })
                }
            };
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

fn parse_curve(s: &str, axes: &[RadarAxis], line_no: usize) -> Result<RadarCurve, ParseError> {
    let s = s.trim();
    // form: id["label"]{v1, v2, ...} or id{v1, v2, ...} or id{ name: v, ... }
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
    let values = parse_curve_values(body, axes, line_no)?;
    Ok(RadarCurve { id, label, values })
}

/// Parse a curve body — either a positional value list (`85, 90, 80`) or
/// `key: value` pairs (`Power: 85, Speed: 90`) matched to axes by id or label.
fn parse_curve_values(
    body: &str,
    axes: &[RadarAxis],
    line_no: usize,
) -> Result<Vec<f64>, ParseError> {
    let is_keyed = body.contains(':');
    if !is_keyed {
        return body
            .split(',')
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| {
                v.parse::<f64>().map_err(|_| ParseError::Syntax {
                    message: format!("invalid value list: '{body}'"),
                    line: line_no,
                })
            })
            .collect();
    }

    let mut map: HashMap<String, f64> = HashMap::new();
    for pair in body.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (name, val) = pair.split_once(':').ok_or_else(|| ParseError::Syntax {
            message: format!("expected 'name: value' in curve entry '{pair}'"),
            line: line_no,
        })?;
        let name = unquote(name.trim()).to_string();
        let val = val.trim().parse::<f64>().map_err(|_| ParseError::Syntax {
            message: format!("invalid curve value: '{}'", val.trim()),
            line: line_no,
        })?;
        map.insert(name, val);
    }
    Ok(axes
        .iter()
        .map(|ax| {
            map.get(&ax.id)
                .or_else(|| map.get(&ax.label))
                .copied()
                .unwrap_or(0.0)
        })
        .collect())
}

fn parse_num(s: &str, what: &str, line_no: usize) -> Result<f64, ParseError> {
    s.parse().map_err(|_| ParseError::Syntax {
        message: format!("invalid {what}: '{s}'"),
        line: line_no,
    })
}

fn parse_bool(s: &str) -> bool {
    !matches!(s, "false" | "0" | "no")
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

    #[test]
    fn multiple_axis_lines_accumulate() {
        let d = parse("radar-beta\naxis A[\"Power\"]\naxis B[\"Speed\"]\ncurve a{1, 2}\n").unwrap();
        assert_eq!(d.axes.len(), 2);
        assert_eq!(d.axes[0].label, "Power");
        assert_eq!(d.axes[1].label, "Speed");
    }

    #[test]
    fn options_do_not_hard_error() {
        let d = parse(
            "radar-beta\naxis A, B\ncurve a{1, 2}\nmin 0\nmax 100\nticks 4\ngraticule polygon\nshowLegend false\n",
        )
        .unwrap();
        assert_eq!(d.min, Some(0.0));
        assert_eq!(d.max, Some(100.0));
        assert_eq!(d.ticks, Some(4));
        assert_eq!(d.graticule, RadarGraticule::Polygon);
        assert_eq!(d.show_legend, Some(false));
    }

    #[test]
    fn keyed_curve_matches_axes_by_name() {
        // Order-independent, matched to axis ids; missing axis defaults to 0.
        let d = parse(
            "radar-beta\naxis a[\"Power\"], b[\"Speed\"], c[\"Endurance\"]\ncurve x{ c: 30, a: 20 }\n",
        )
        .unwrap();
        assert_eq!(d.curves[0].values, vec![20.0, 0.0, 30.0]);
    }

    #[test]
    fn keyed_curve_matches_axis_label() {
        let d =
            parse("radar-beta\naxis a[\"Power\"], b[\"Speed\"]\ncurve x{ Speed: 5, Power: 9 }\n")
                .unwrap();
        assert_eq!(d.curves[0].values, vec![9.0, 5.0]);
    }
}
