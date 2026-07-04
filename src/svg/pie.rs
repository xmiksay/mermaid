//! Pie chart renderer.

use std::fmt::Write as _;

use crate::parse::PieDiagram;

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const RADIUS: f64 = 150.0;
const PAD: f64 = 24.0;
const TITLE_GAP: f64 = 30.0;
const LEGEND_ROW: f64 = 22.0;
const SWATCH: f64 = 14.0;
const LEGEND_LABEL_MAX: usize = 24;
/// Slices below this fraction of the total are dropped, matching upstream
/// `createPieArcs`.
const MIN_SLICE: f64 = 0.01;
/// Gap between the pie and a top/bottom legend block.
const LEGEND_GAP: f64 = 12.0;

/// Where the legend sits relative to the pie (`config.pie.legendPosition`).
#[derive(Clone, Copy, PartialEq)]
enum LegendPos {
    Right,
    Left,
    Top,
    Bottom,
}

impl LegendPos {
    fn parse(s: Option<&str>) -> Self {
        match s.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("left") => LegendPos::Left,
            Some("top") => LegendPos::Top,
            Some("bottom") => LegendPos::Bottom,
            _ => LegendPos::Right,
        }
    }
}

pub(crate) fn render(p: &PieDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;
    let title_color = theme.title();
    let pie_stroke = theme.pie_stroke();
    // `pieOpacity` emits a `fill-opacity` attribute only when set, so the
    // default render stays byte-identical.
    let opacity_attr = match &theme.pie_opacity {
        Some(o) => format!(" fill-opacity=\"{o}\""),
        None => String::new(),
    };
    let pie_color = |i| theme.pie_color(i);

    let total: f64 = p.entries.iter().map(|e| e.value.max(0.0)).sum();

    // Upstream `createPieArcs` drops slices under 1% of the total (percentages
    // stay relative to the full total). Insertion order is preserved — d3 uses
    // `.sort(null)` — and the original index keeps each slice's palette color.
    let shown: Vec<(usize, &crate::parse::PieEntry)> = p
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| total <= 0.0 || e.value.max(0.0) / total >= MIN_SLICE)
        .collect();

    // Config knobs (upstream defaults: textPosition 0.75, donutHole 0, right).
    let text_pos = p.text_position.unwrap_or(0.75);
    let inner_r = RADIUS * p.donut_hole.unwrap_or(0.0).clamp(0.0, 0.95);
    let legend_pos = LegendPos::parse(p.legend_position.as_deref());

    let legend_w = if shown.is_empty() {
        0.0
    } else {
        let longest = shown
            .iter()
            .map(|(_, e)| label_len(&legend_text(e, total, p.show_data)))
            .max()
            .unwrap_or(0);
        // ~7 px per char for sans-serif 14px at the legend size, + swatch + padding
        (longest.min(LEGEND_LABEL_MAX) as f64) * 8.0 + SWATCH + 16.0
    };
    let legend_h = shown.len() as f64 * LEGEND_ROW;

    let title_h = if p.title.is_some() { TITLE_GAP } else { 0.0 };

    // Lay out the pie centre, canvas size, and legend origin per legend position.
    let (width, height, cx, cy, legend_x, legend_top) = match legend_pos {
        LegendPos::Right => {
            let cy = PAD + title_h + RADIUS;
            (
                PAD * 2.0 + RADIUS * 2.0 + legend_w,
                PAD * 2.0 + title_h + RADIUS * 2.0,
                PAD + RADIUS,
                cy,
                PAD + RADIUS * 2.0 + 8.0,
                cy - legend_h / 2.0,
            )
        }
        LegendPos::Left => {
            let cy = PAD + title_h + RADIUS;
            (
                PAD * 2.0 + RADIUS * 2.0 + legend_w,
                PAD * 2.0 + title_h + RADIUS * 2.0,
                PAD + legend_w + RADIUS,
                cy,
                PAD,
                cy - legend_h / 2.0,
            )
        }
        LegendPos::Bottom => {
            let width = PAD * 2.0 + (RADIUS * 2.0).max(legend_w);
            (
                width,
                PAD * 2.0 + title_h + RADIUS * 2.0 + LEGEND_GAP + legend_h,
                width / 2.0,
                PAD + title_h + RADIUS,
                (width - legend_w) / 2.0,
                PAD + title_h + RADIUS * 2.0 + LEGEND_GAP,
            )
        }
        LegendPos::Top => {
            let width = PAD * 2.0 + (RADIUS * 2.0).max(legend_w);
            (
                width,
                PAD * 2.0 + title_h + RADIUS * 2.0 + LEGEND_GAP + legend_h,
                width / 2.0,
                PAD + title_h + legend_h + LEGEND_GAP + RADIUS,
                (width - legend_w) / 2.0,
                PAD + title_h,
            )
        }
    };

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &p.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{title_color}\" font-size=\"18\" font-weight=\"bold\""
            ),
            t,
        );
    }

    if total <= 0.0 || shown.is_empty() {
        // Empty pie: just draw the outline so the diagram is identifiable.
        svg.circle(
            cx,
            cy,
            RADIUS,
            &format!("fill=\"none\" stroke=\"{fg_muted}\" stroke-width=\"1\""),
        );
        return svg.finish();
    }

    // Sweep segments starting at angle = -π/2 (top, like most pie charts).
    let mut angle = -std::f64::consts::FRAC_PI_2;
    for &(i, e) in &shown {
        let frac = e.value.max(0.0) / total;
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f64::consts::TAU;
        let end = angle + sweep;
        let large = if sweep > std::f64::consts::PI { 1 } else { 0 };

        // Full-circle case: SVG arc can't draw a 360° arc with one path; split.
        let segs: Vec<(f64, f64)> = if frac >= 0.9999 {
            vec![
                (angle, angle + std::f64::consts::PI),
                (angle + std::f64::consts::PI, end),
            ]
        } else {
            vec![(angle, end)]
        };

        for (a1, a2) in segs {
            svg.path(
                &slice_path(cx, cy, RADIUS, inner_r, a1, a2, large),
                &format!(
                    "fill=\"{c}\"{opacity_attr} stroke=\"{pie_stroke}\" stroke-width=\"1\"",
                    c = pie_color(i)
                ),
            );
        }

        // Upstream draws the integer percentage on each slice, at
        // `textPosition`·radius along the slice's mid-angle (`labelArc`
        // centroid, default `textPosition` 0.75).
        let mid = angle + sweep / 2.0;
        let lr = RADIUS * text_pos;
        let pct = (frac * 100.0).round();
        svg.text(
            cx + lr * mid.cos(),
            cy + lr * mid.sin(),
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
            &format!("{}%", fnum(pct)),
        );

        angle = end;
    }

    // Legend
    for (row, &(i, e)) in shown.iter().enumerate() {
        let y = legend_top + row as f64 * LEGEND_ROW;
        svg.rect(
            legend_x,
            y,
            SWATCH,
            SWATCH,
            &format!(
                "fill=\"{c}\" stroke=\"{pie_stroke}\" stroke-width=\"1\"",
                c = pie_color(i)
            ),
        );
        let label = legend_text(e, total, p.show_data);
        let label = truncate(&label, LEGEND_LABEL_MAX);
        svg.text(
            legend_x + SWATCH + 6.0,
            y + SWATCH - 2.0,
            &format!("fill=\"{fg}\" font-size=\"13\""),
            &label,
        );
    }

    svg.finish()
}

/// Build the SVG path for one slice between angles `a1`..`a2`. A zero inner
/// radius draws a full wedge (center → arc → center); a positive `ir` draws an
/// annular sector (outer arc, in, inner arc back), i.e. a donut slice.
fn slice_path(cx: f64, cy: f64, r: f64, ir: f64, a1: f64, a2: f64, large: u8) -> String {
    let (ox1, oy1) = (cx + r * a1.cos(), cy + r * a1.sin());
    let (ox2, oy2) = (cx + r * a2.cos(), cy + r * a2.sin());
    let mut d = String::new();
    if ir <= 0.0 {
        let _ = write!(
            d,
            "M{cx} {cy}L{ox1} {oy1}A{r} {r} 0 {large} 1 {ox2} {oy2}Z",
            cx = fnum(cx),
            cy = fnum(cy),
            ox1 = fnum(ox1),
            oy1 = fnum(oy1),
            r = fnum(r),
            large = large,
            ox2 = fnum(ox2),
            oy2 = fnum(oy2),
        );
    } else {
        let (ix1, iy1) = (cx + ir * a1.cos(), cy + ir * a1.sin());
        let (ix2, iy2) = (cx + ir * a2.cos(), cy + ir * a2.sin());
        let _ = write!(
            d,
            "M{ox1} {oy1}A{r} {r} 0 {large} 1 {ox2} {oy2}L{ix2} {iy2}A{ir} {ir} 0 {large} 0 {ix1} {iy1}Z",
            ox1 = fnum(ox1),
            oy1 = fnum(oy1),
            r = fnum(r),
            large = large,
            ox2 = fnum(ox2),
            oy2 = fnum(oy2),
            ix2 = fnum(ix2),
            iy2 = fnum(iy2),
            ir = fnum(ir),
            ix1 = fnum(ix1),
            iy1 = fnum(iy1),
        );
    }
    d
}

/// Legend entry text. Upstream keeps legend entries to the bare label; the
/// value is appended only under `showData` (`label [value]`). Percentages live
/// on the slices, not the legend.
fn legend_text(e: &crate::parse::PieEntry, _total: f64, show_data: bool) -> String {
    if show_data {
        format!("{} [{}]", e.label, fnum(e.value))
    } else {
        e.label.clone()
    }
}

fn label_len(s: &str) -> usize {
    s.chars().count()
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let mut out: String = chars[..n.saturating_sub(1)].iter().collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::PieEntry;

    fn pie(entries: Vec<(&str, f64)>) -> PieDiagram {
        PieDiagram {
            title: Some("test".into()),
            show_data: false,
            entries: entries
                .into_iter()
                .map(|(l, v)| PieEntry {
                    label: l.into(),
                    value: v,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn produces_svg_envelope() {
        let svg = render(&pie(vec![("A", 1.0), ("B", 1.0)]), &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn contains_one_path_per_segment() {
        let svg = render(
            &pie(vec![("A", 1.0), ("B", 1.0), ("C", 1.0)]),
            &Theme::default(),
        );
        assert_eq!(svg.matches("<path").count(), 3);
    }

    #[test]
    fn contains_title_and_legend_labels() {
        let svg = render(
            &pie(vec![("Chrome", 60.0), ("Firefox", 40.0)]),
            &Theme::default(),
        );
        assert!(svg.contains("test"));
        assert!(svg.contains("Chrome"));
        assert!(svg.contains("Firefox"));
        // percentage rendered in legend
        assert!(svg.contains("60%"));
    }

    #[test]
    fn percentage_on_slices_not_legend() {
        // Upstream draws the percentage on each slice; legend stays bare labels.
        let svg = render(
            &pie(vec![("Chrome", 60.0), ("Firefox", 40.0)]),
            &Theme::default(),
        );
        assert!(svg.contains("60%"), "slice percentage rendered");
        assert!(svg.contains("40%"));
        assert!(svg.contains(">Chrome</text>"), "legend label bare");
        assert!(!svg.contains("Chrome (60%)"), "no percentage in legend");
    }

    #[test]
    fn show_data_appends_value_to_legend() {
        let mut p = pie(vec![("Chrome", 60.0), ("Firefox", 40.0)]);
        p.show_data = true;
        let svg = render(&p, &Theme::default());
        assert!(svg.contains("Chrome [60]"), "value shown with showData");
    }

    #[test]
    fn handles_full_circle() {
        // Single segment = 100%: must split into two arcs internally.
        let svg = render(&pie(vec![("All", 1.0)]), &Theme::default());
        // Two path segments for full circle.
        assert_eq!(svg.matches("<path").count(), 2);
    }

    #[test]
    fn drops_slices_below_one_percent() {
        // C is 0.5% of the total (< 1%) → filtered from arcs and legend, like
        // upstream `createPieArcs`.
        let svg = render(
            &pie(vec![("A", 100.0), ("B", 99.5), ("C", 1.0)]),
            &Theme::default(),
        );
        assert_eq!(svg.matches("<path").count(), 2, "tiny slice dropped");
        assert!(svg.contains(">A</text>"));
        assert!(svg.contains(">B</text>"));
        assert!(!svg.contains(">C</text>"), "tiny slice absent from legend");
    }

    #[test]
    fn donut_hole_cuts_annular_slices() {
        let mut p = pie(vec![("A", 1.0), ("B", 1.0)]);
        p.donut_hole = Some(0.5);
        let svg = render(&p, &Theme::default());
        // Annular sectors use two arcs per slice (no `M<cx> <cy>L` wedge apex).
        assert!(svg.contains("A150 150"), "outer arc present");
        assert!(svg.contains("A75 75"), "inner (hole) arc present");
    }

    #[test]
    fn text_position_moves_slice_labels() {
        let mut near = pie(vec![("A", 1.0)]);
        near.text_position = Some(0.2);
        let mut far = pie(vec![("A", 1.0)]);
        far.text_position = Some(0.9);
        // A single 100%-slice puts its label straight up from the centre; a
        // larger textPosition pushes it further from the centre (smaller y).
        let near_svg = render(&near, &Theme::default());
        let far_svg = render(&far, &Theme::default());
        assert_ne!(near_svg, far_svg, "textPosition changes label placement");
    }

    #[test]
    fn legend_position_bottom_changes_layout() {
        let base = render(&pie(vec![("A", 1.0), ("B", 1.0)]), &Theme::default());
        let mut p = pie(vec![("A", 1.0), ("B", 1.0)]);
        p.legend_position = Some("bottom".into());
        let bottom = render(&p, &Theme::default());
        assert_ne!(base, bottom, "bottom legend produces a different layout");
    }

    #[test]
    fn default_config_is_unchanged() {
        // No config → byte-identical to the pre-config renderer (donutHole 0,
        // textPosition 0.75, right legend).
        let svg = render(&pie(vec![("A", 1.0), ("B", 1.0)]), &Theme::default());
        assert!(svg.contains("M174 204L"), "full wedge apex at the centre");
        assert!(!svg.contains("A75 75"), "no donut hole by default");
    }

    #[test]
    fn handles_empty() {
        let svg = render(&PieDiagram::default(), &Theme::default());
        assert!(svg.starts_with("<svg"));
        // No segments, just outline circle
        assert!(svg.contains("<circle"));
    }
}
