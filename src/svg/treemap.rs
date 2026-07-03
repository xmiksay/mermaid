//! Treemap renderer. Squarified layout (Bruls/Huizing/van Wijk greedy
//! worst-aspect-ratio row packing) matching upstream d3 treemaps — rectangles
//! stay near square instead of degenerating into long thin slivers. Leaf values
//! are formatted through the `valueFormat` d3-format subset.

use std::collections::HashMap;

use crate::parse::ast::Style;
use crate::parse::{TreemapDiagram, TreemapNode};

use super::builder::{fnum, SvgBuilder};
use super::style::resolve_style;
use super::theme::Theme;

const PAD: f64 = 24.0;
const TITLE_GAP: f64 = 32.0;
const CHART_W: f64 = 640.0;
const CHART_H: f64 = 420.0;
const HEADER_H: f64 = 22.0;

/// Shared, read-only context threaded through the recursive layout.
struct Ctx<'a> {
    theme: &'a Theme,
    class_defs: &'a HashMap<String, Style>,
    value_format: Option<&'a str>,
}

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

pub(crate) fn render(d: &TreemapDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let width = PAD * 2.0 + CHART_W;
    let height = PAD * 2.0 + title_h + CHART_H;
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let ctx = Ctx {
        theme,
        class_defs: &d.class_defs,
        value_format: d.value_format.as_deref(),
    };
    let area = Rect {
        x: PAD,
        y: PAD + title_h,
        w: CHART_W,
        h: CHART_H,
    };
    layout(&d.root, area, 0, &mut svg, &ctx);

    svg.finish()
}

fn node_value(n: &TreemapNode) -> f64 {
    if let Some(v) = n.value {
        return v;
    }
    let s: f64 = n.children.iter().map(node_value).sum();
    if s == 0.0 {
        1.0
    } else {
        s
    }
}

fn layout(nodes: &[TreemapNode], area: Rect, depth: usize, svg: &mut SvgBuilder, ctx: &Ctx) {
    if nodes.is_empty() || area.w <= 2.0 || area.h <= 2.0 {
        return;
    }
    let values: Vec<f64> = nodes.iter().map(node_value).collect();
    let rects = squarify(&values, area);
    for (i, (n, r)) in nodes.iter().zip(rects.iter()).enumerate() {
        draw_node(n, *r, i, depth, svg, ctx);
        if !n.children.is_empty() && r.w > 30.0 && r.h > HEADER_H + 10.0 {
            let inner = Rect {
                x: r.x + 4.0,
                y: r.y + HEADER_H,
                w: r.w - 8.0,
                h: r.h - HEADER_H - 4.0,
            };
            layout(&n.children, inner, depth + 1, svg, ctx);
        }
    }
}

fn draw_node(n: &TreemapNode, r: Rect, i: usize, depth: usize, svg: &mut SvgBuilder, ctx: &Ctx) {
    // A `:::class` reference overrides the palette fill/stroke.
    let classes: Vec<String> = n.class_name.iter().cloned().collect();
    let rs = resolve_style(ctx.class_defs, &classes, &Style::new());
    let color = rs
        .fill
        .clone()
        .unwrap_or_else(|| ctx.theme.pie_color(i + depth).to_string());
    let stroke = rs.stroke.as_deref().unwrap_or("#fff");
    let leaf = n.children.is_empty();
    svg.rect(
        r.x,
        r.y,
        r.w,
        r.h,
        &format!(
            "fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"{stroke}\" stroke-width=\"1.5\"",
            op = if leaf { "0.85" } else { "0.25" }
        ),
    );
    if r.w > 24.0 && r.h > 16.0 {
        let font_size = if r.w < 60.0 { 10 } else { 12 };
        let label_fill: &str = if leaf { "#fff" } else { &ctx.theme.fg };
        svg.text(
            r.x + 4.0,
            r.y + 12.0,
            &format!(
                "fill=\"{}\" font-size=\"{font_size}\" font-weight=\"bold\"",
                label_fill
            ),
            &n.label,
        );
        if let Some(v) = n.value {
            if leaf && r.h > 28.0 {
                svg.text(
                    r.x + 4.0,
                    r.y + 24.0,
                    "fill=\"#fff\" font-size=\"9\"",
                    &format_value(v, ctx.value_format),
                );
            }
        }
    }
}

/// Squarified treemap: pack `values` into `area`, one output rect per value, in
/// input order, keeping rows near square by the worst-aspect-ratio heuristic.
fn squarify(values: &[f64], area: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    if values.is_empty() || total <= 0.0 || area.w <= 0.0 || area.h <= 0.0 {
        return vec![area; values.len()];
    }
    // Scale values so their sum equals the rectangle's area; then row lengths
    // fall directly out of the packed sub-areas.
    let scale = (area.w * area.h) / total;
    let areas: Vec<f64> = values.iter().map(|v| v * scale).collect();

    let mut out = Vec::with_capacity(values.len());
    let (mut x, mut y, mut w, mut h) = (area.x, area.y, area.w, area.h);
    let mut i = 0;
    while i < areas.len() {
        let short = w.min(h);
        // Greedily extend the current row while the worst aspect ratio improves.
        let mut end = i + 1;
        let mut best = worst(&areas[i..end], short);
        while end < areas.len() {
            let cand = worst(&areas[i..end + 1], short);
            if cand > best {
                break;
            }
            best = cand;
            end += 1;
        }
        let row = &areas[i..end];
        let row_sum: f64 = row.iter().sum();
        if w <= h {
            // Horizontal row across the top of the remaining area.
            let row_h = row_sum / w;
            let mut rx = x;
            for &a in row {
                let rw = a / row_h;
                out.push(Rect {
                    x: rx,
                    y,
                    w: rw,
                    h: row_h,
                });
                rx += rw;
            }
            y += row_h;
            h -= row_h;
        } else {
            // Vertical column down the left of the remaining area.
            let col_w = row_sum / h;
            let mut ry = y;
            for &a in row {
                let rh = a / col_w;
                out.push(Rect {
                    x,
                    y: ry,
                    w: col_w,
                    h: rh,
                });
                ry += rh;
            }
            x += col_w;
            w -= col_w;
        }
        i = end;
    }
    out
}

/// Worst (largest) aspect ratio produced by laying `row` along a side of
/// length `side` — the Bruls/Huizing/van Wijk objective.
fn worst(row: &[f64], side: f64) -> f64 {
    let s: f64 = row.iter().sum();
    if s <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let rmax = row.iter().cloned().fold(f64::MIN, f64::max);
    let rmin = row.iter().cloned().fold(f64::MAX, f64::min);
    let side2 = side * side;
    let s2 = s * s;
    (side2 * rmax / s2).max(s2 / (side2 * rmin))
}

/// Format a leaf value through the supported `valueFormat` subset: `$` prefix,
/// `,` thousands grouping, `.N` decimal places, `%` percentage. Absent a
/// format, the value is rendered naturally.
fn format_value(v: f64, fmt: Option<&str>) -> String {
    let Some(fmt) = fmt else {
        return fnum(v);
    };
    let currency = fmt.contains('$');
    let percent = fmt.contains('%');
    let thousands = fmt.contains(',');
    let decimals = fmt.split_once('.').map(|(_, rest)| {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<usize>().unwrap_or(0)
    });

    let val = if percent { v * 100.0 } else { v };
    let mut body = match decimals {
        Some(d) => format!("{val:.d$}"),
        None => fnum(val),
    };
    if thousands {
        body = group_thousands(&body);
    }
    let mut out = String::new();
    if currency {
        out.push('$');
    }
    out.push_str(&body);
    if percent {
        out.push('%');
    }
    out
}

/// Insert `,` thousands separators into the integer part of a numeric string,
/// preserving any sign and fractional part.
fn group_thousands(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let (int_part, frac) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    let len = int_part.chars().count();
    let mut grouped = String::with_capacity(len + len / 3);
    for (idx, ch) in int_part.chars().enumerate() {
        if idx > 0 && (len - idx) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let mut out = format!("{sign}{grouped}");
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(label: &str, value: f64) -> TreemapNode {
        TreemapNode {
            label: label.into(),
            value: Some(value),
            children: vec![],
            class_name: None,
        }
    }

    #[test]
    fn produces_svg() {
        let d = TreemapDiagram {
            title: Some("Tree".into()),
            root: vec![TreemapNode {
                label: "A".into(),
                value: None,
                children: vec![leaf("A1", 3.0), leaf("A2", 7.0)],
                class_name: None,
            }],
            class_defs: HashMap::new(),
            value_format: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Tree<"));
        assert!(svg.contains(">A1<"));
    }

    #[test]
    fn class_fill_overrides_palette() {
        let mut class_defs = HashMap::new();
        class_defs.insert(
            "hot".to_string(),
            vec![("fill".to_string(), "#ff0000".to_string())],
        );
        let d = TreemapDiagram {
            title: None,
            root: vec![TreemapNode {
                label: "A".into(),
                value: Some(5.0),
                children: vec![],
                class_name: Some("hot".into()),
            }],
            class_defs,
            value_format: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(
            svg.contains("fill=\"#ff0000\""),
            "class fill not applied: {svg}"
        );
        assert!(!svg.contains(":::"));
    }

    #[test]
    fn squarify_tiles_the_whole_area() {
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let values = vec![6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
        let rects = squarify(&values, area);
        assert_eq!(rects.len(), values.len());
        let covered: f64 = rects.iter().map(|r| r.w * r.h).sum();
        assert!((covered - 100.0 * 100.0).abs() < 1e-6, "area mismatch");
        // Every rect stays inside the area.
        for r in &rects {
            assert!(r.x >= -1e-6 && r.y >= -1e-6);
            assert!(r.x + r.w <= 100.0 + 1e-6 && r.y + r.h <= 100.0 + 1e-6);
        }
    }

    #[test]
    fn squarify_keeps_reasonable_aspect_ratios() {
        // Slice-and-dice would give each equal value a 100x(100/8) sliver
        // (ratio 8); squarify must do far better.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let values = vec![1.0; 8];
        let rects = squarify(&values, area);
        for r in &rects {
            let ratio = (r.w / r.h).max(r.h / r.w);
            assert!(ratio < 3.0, "sliver aspect ratio {ratio}");
        }
    }

    #[test]
    fn value_format_subset() {
        assert_eq!(format_value(1234.0, Some("$0,0")), "$1,234");
        assert_eq!(format_value(1234.567, Some(",.2f")), "1,234.57");
        assert_eq!(format_value(0.42, Some(".1%")), "42.0%");
        assert_eq!(format_value(0.42, Some("%")), "42%");
        assert_eq!(format_value(1000.0, Some("$,.2f")), "$1,000.00");
        assert_eq!(format_value(-1234.0, Some(",")), "-1,234");
        // No format → natural rendering.
        assert_eq!(format_value(12.0, None), "12");
    }
}
