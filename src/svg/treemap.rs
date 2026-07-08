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
    show_values: bool,
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
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\""),
            t,
        );
    }

    let ctx = Ctx {
        theme,
        class_defs: &d.class_defs,
        value_format: d.value_format.as_deref(),
        show_values: d.show_values != Some(false),
    };
    let area = Rect {
        x: PAD,
        y: PAD + title_h,
        w: CHART_W,
        h: CHART_H,
    };
    let mut next_id = 0usize;
    let mut next_color = 0usize;
    layout(
        &d.root,
        area,
        None,
        &mut svg,
        &ctx,
        &mut next_id,
        &mut next_color,
    );

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

/// Order sibling indices by value descending, ties keeping source order
/// (`sort_by` is stable). Upstream sorts every level this way.
fn order_by_value(nodes: &[TreemapNode]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..nodes.len()).collect();
    idx.sort_by(|&a, &b| node_value(&nodes[b]).total_cmp(&node_value(&nodes[a])));
    idx
}

fn layout(
    nodes: &[TreemapNode],
    area: Rect,
    parent_color: Option<&str>,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
    next_color: &mut usize,
) {
    if nodes.is_empty() || area.w <= 2.0 || area.h <= 2.0 {
        return;
    }
    let order = order_by_value(nodes);
    let values: Vec<f64> = order.iter().map(|&i| node_value(&nodes[i])).collect();
    let rects = squarify(&values, area);
    for (&i, r) in order.iter().zip(rects.iter()) {
        let n = &nodes[i];
        // Every section takes the next palette hue in traversal order; its
        // leaves inherit that hue uniformly. A top-level leaf (no parent
        // section) also gets its own hue. This matches upstream, where each
        // branch is one flat color and nested sections switch hue.
        let color = match (n.children.is_empty(), parent_color) {
            (true, Some(pc)) => pc.to_string(),
            _ => {
                let c = ctx.theme.cscale_color(*next_color).to_string();
                *next_color += 1;
                c
            }
        };
        draw_node(n, *r, &color, svg, ctx, next_id);
        if !n.children.is_empty() && r.w > 30.0 && r.h > HEADER_H + 10.0 {
            let inner = Rect {
                x: r.x + 4.0,
                y: r.y + HEADER_H,
                w: r.w - 8.0,
                h: r.h - HEADER_H - 4.0,
            };
            layout(
                &n.children,
                inner,
                Some(&color),
                svg,
                ctx,
                next_id,
                next_color,
            );
        }
    }
}

fn draw_node(
    n: &TreemapNode,
    r: Rect,
    color: &str,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
) {
    let leaf = n.children.is_empty();
    // A `:::class` reference overrides the branch fill/stroke.
    let classes: Vec<String> = n.class_name.iter().cloned().collect();
    let rs = resolve_style(ctx.class_defs, &classes, &Style::new());
    // Sections and their leaves share one flat hue (upstream draws no
    // per-sibling shading); white strokes keep adjacent cells legible.
    let fill = rs.fill.clone().unwrap_or_else(|| color.to_string());
    let stroke = rs.stroke.as_deref().unwrap_or("#ffffff");
    svg.rect(
        r.x,
        r.y,
        r.w,
        r.h,
        &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\""),
    );
    let ink = text_color(&fill, ctx.theme);
    if leaf {
        draw_leaf_label(n, r, &ink, svg, ctx, next_id);
    } else {
        draw_section_header(n, r, &ink, svg, ctx, next_id);
    }
}

/// Pick white or the theme foreground for text drawn on `fill`, by luminance —
/// upstream uses white on its darker section fills, dark ink on light ones.
fn text_color(fill: &str, theme: &Theme) -> String {
    match parse_hex(fill) {
        Some((r, g, b)) => {
            let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            if lum < 140.0 {
                "#ffffff".to_string()
            } else {
                theme.fg.to_string()
            }
        }
        None => theme.fg.to_string(),
    }
}

/// Centered name over its value, clipped to the cell.
fn draw_leaf_label(
    n: &TreemapNode,
    r: Rect,
    ink: &str,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
) {
    if r.w <= 24.0 || r.h <= 16.0 {
        return;
    }
    let value_text = if ctx.show_values && r.h > 30.0 {
        n.value.map(|v| format_value(v, ctx.value_format))
    } else {
        None
    };
    let cx = r.x + r.w / 2.0;
    let cy = r.y + r.h / 2.0;
    let clip = clip_open(r, svg, next_id);
    let name_y = if value_text.is_some() {
        cy - 2.0
    } else {
        cy + 5.0
    };
    svg.text(
        cx,
        name_y,
        &format!("text-anchor=\"middle\" fill=\"{ink}\" font-size=\"16\""),
        &n.label,
    );
    if let Some(v) = value_text {
        svg.text(
            cx,
            cy + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{ink}\" font-size=\"12\""),
            &v,
        );
    }
    clip_close(clip, svg);
}

/// Section band: name left-aligned, running total right-aligned in italics.
fn draw_section_header(
    n: &TreemapNode,
    r: Rect,
    ink: &str,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
) {
    if r.w <= 30.0 || r.h <= 16.0 {
        return;
    }
    let clip = clip_open(r, svg, next_id);
    let y = r.y + 15.0;
    svg.text(
        r.x + 6.0,
        y,
        &format!("text-anchor=\"start\" fill=\"{ink}\" font-size=\"13\" font-weight=\"bold\""),
        &n.label,
    );
    if ctx.show_values {
        svg.text(
            r.x + r.w - 6.0,
            y,
            &format!("text-anchor=\"end\" fill=\"{ink}\" font-size=\"12\" font-style=\"italic\""),
            &format_value(node_value(n), ctx.value_format),
        );
    }
    clip_close(clip, svg);
}

/// Register a per-cell clip path and open a `<g>` bound to it. Returns the id.
fn clip_open(r: Rect, svg: &mut SvgBuilder, next_id: &mut usize) -> usize {
    let id = *next_id;
    *next_id += 1;
    svg.defs_raw(&format!(
        "<clipPath id=\"tm-clip-{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
        fnum(r.x),
        fnum(r.y),
        fnum(r.w),
        fnum(r.h)
    ));
    svg.raw(&format!("<g clip-path=\"url(#tm-clip-{id})\">"));
    id
}

fn clip_close(_id: usize, svg: &mut SvgBuilder) {
    svg.raw("</g>");
}

/// Parse `#rgb`/`#rrggbb` into RGB bytes; `None` for any other syntax.
fn parse_hex(c: &str) -> Option<(u8, u8, u8)> {
    let h = c.strip_prefix('#')?;
    let (r, g, b) = match h.len() {
        6 => (&h[0..2], &h[2..4], &h[4..6]),
        3 => {
            return parse_hex(&format!(
                "#{a}{a}{b}{b}{c}{c}",
                a = &h[0..1],
                b = &h[1..2],
                c = &h[2..3]
            ))
        }
        _ => return None,
    };
    Some((
        u8::from_str_radix(r, 16).ok()?,
        u8::from_str_radix(g, 16).ok()?,
        u8::from_str_radix(b, 16).ok()?,
    ))
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
/// `,` thousands grouping, `.N` decimal places, `%` percentage. Upstream
/// defaults `valueFormat` to `,` (thousands grouping) when unset.
fn format_value(v: f64, fmt: Option<&str>) -> String {
    let fmt = fmt.unwrap_or(",");
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
            show_values: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        // Title uses upstream regular weight, not bold (#332).
        assert!(svg.contains("font-size=\"18\">Tree</text>"));
        assert!(!svg.contains("font-weight=\"bold\">Tree<"));
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
            show_values: None,
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
        // No format → upstream default ',' thousands grouping.
        assert_eq!(format_value(12.0, None), "12");
        assert_eq!(format_value(1234567.0, None), "1,234,567");
    }

    #[test]
    fn show_values_false_hides_leaf_value() {
        let d = TreemapDiagram {
            title: None,
            root: vec![leaf("Big", 1234.0)],
            class_defs: HashMap::new(),
            value_format: None,
            show_values: Some(false),
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">Big<"), "label should still render: {svg}");
        assert!(
            !svg.contains(">1,234<"),
            "leaf value should be hidden: {svg}"
        );
    }

    #[test]
    fn orders_siblings_by_value_desc() {
        // Source order Hot(60) then Cold(65); sorted rank puts Cold first.
        let nodes = vec![leaf("Hot", 60.0), leaf("Cold", 65.0)];
        assert_eq!(order_by_value(&nodes), vec![1, 0]);
    }

    #[test]
    fn text_color_flips_on_luminance() {
        let t = Theme::default();
        // Light pastel → dark theme ink; dark fill → white.
        assert_eq!(text_color("#B9B9FF", &t), t.fg);
        assert_eq!(text_color("#101010", &t), "#ffffff");
        // Non-hex passes through to the theme foreground.
        assert_eq!(text_color("url(#g)", &t), t.fg);
    }

    #[test]
    fn each_section_gets_its_own_hue_leaves_uniform() {
        let section = |label: &str, kids: Vec<TreemapNode>| TreemapNode {
            label: label.into(),
            value: None,
            children: kids,
            class_name: None,
        };
        let d = TreemapDiagram {
            title: None,
            root: vec![
                // Cold(65) sorts before Hot(60); Hot nests a Tea section.
                section("Cold", vec![leaf("Water", 40.0), leaf("Soda", 25.0)]),
                section(
                    "Hot",
                    vec![
                        leaf("Coffee", 35.0),
                        section("Tea", vec![leaf("Black", 12.0), leaf("Green", 8.0)]),
                    ],
                ),
            ],
            class_defs: HashMap::new(),
            value_format: None,
            show_values: None,
        };
        let svg = render(&d, &Theme::default());
        // Cold = cScale0, Hot = cScale1, nested Tea takes its own cScale2 hue
        // rather than inheriting Hot's yellow.
        assert!(svg.contains("fill=\"#B9B9FF\""), "Cold hue missing: {svg}");
        assert!(svg.contains("fill=\"#FFFFAB\""), "Hot hue missing: {svg}");
        assert!(
            svg.contains("fill=\"#E8FFB9\""),
            "nested Tea hue missing: {svg}"
        );
        // No progressive darkening: siblings never step to an off-palette shade.
        assert!(
            !svg.contains("fill=\"#A6A6E6"),
            "sibling darkening leaked: {svg}"
        );
    }

    #[test]
    fn section_header_shows_total() {
        let d = TreemapDiagram {
            title: None,
            root: vec![TreemapNode {
                label: "Cold".into(),
                value: None,
                children: vec![leaf("Water", 40.0), leaf("Soda", 25.0)],
                class_name: None,
            }],
            class_defs: HashMap::new(),
            value_format: None,
            show_values: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(
            svg.contains("font-style=\"italic\""),
            "header total not italic: {svg}"
        );
        assert!(svg.contains(">65<"), "section total 65 missing: {svg}");
    }

    #[test]
    fn default_value_format_groups_thousands() {
        let d = TreemapDiagram {
            title: None,
            root: vec![leaf("Big", 1234567.0)],
            class_defs: HashMap::new(),
            value_format: None,
            show_values: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(
            svg.contains(">1,234,567<"),
            "default valueFormat should group thousands: {svg}"
        );
    }
}
