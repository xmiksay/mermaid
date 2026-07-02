//! Pie chart renderer.

use std::fmt::Write as _;

use crate::parse::PieDiagram;

use super::builder::{escape, fnum, SvgBuilder};
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

pub(crate) fn render(p: &PieDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
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

    let cx = PAD + RADIUS;
    let title_h = if p.title.is_some() { TITLE_GAP } else { 0.0 };
    let cy = PAD + title_h + RADIUS;
    let width = PAD * 2.0 + RADIUS * 2.0 + legend_w;
    let height = PAD * 2.0 + title_h + RADIUS * 2.0;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    if let Some(t) = &p.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
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
            let (x1, y1) = (cx + RADIUS * a1.cos(), cy + RADIUS * a1.sin());
            let (x2, y2) = (cx + RADIUS * a2.cos(), cy + RADIUS * a2.sin());
            let mut d = String::new();
            let _ = write!(
                d,
                "M{cx} {cy}L{x1} {y1}A{r} {r} 0 {large} 1 {x2} {y2}Z",
                cx = fnum(cx),
                cy = fnum(cy),
                x1 = fnum(x1),
                y1 = fnum(y1),
                x2 = fnum(x2),
                y2 = fnum(y2),
                large = large,
                r = fnum(RADIUS)
            );
            svg.path(
                &d,
                &format!(
                    "fill=\"{c}\" stroke=\"#fff\" stroke-width=\"1\"",
                    c = pie_color(i)
                ),
            );
        }
        angle = end;
    }

    // Legend
    let legend_x = PAD + RADIUS * 2.0 + 8.0;
    let legend_top = PAD + title_h + RADIUS - (shown.len() as f64 * LEGEND_ROW) / 2.0;
    for (row, &(i, e)) in shown.iter().enumerate() {
        let y = legend_top + row as f64 * LEGEND_ROW;
        svg.rect(
            legend_x,
            y,
            SWATCH,
            SWATCH,
            &format!(
                "fill=\"{c}\" stroke=\"#fff\" stroke-width=\"1\"",
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

fn legend_text(e: &crate::parse::PieEntry, total: f64, show_data: bool) -> String {
    if show_data {
        let pct = e.value.max(0.0) / total * 100.0;
        format!(
            "{} ({} | {}%)",
            e.label,
            fnum(e.value),
            fnum(round_pct(pct))
        )
    } else {
        let pct = e.value.max(0.0) / total * 100.0;
        format!("{} ({}%)", e.label, fnum(round_pct(pct)))
    }
}

fn round_pct(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
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

// escape is re-exported in case callers want it; suppress unused warning.
#[allow(dead_code)]
fn _use_escape(s: &str) -> String {
    escape(s)
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
        assert!(svg.contains("A ("));
        assert!(svg.contains("B ("));
        assert!(!svg.contains("C ("), "tiny slice absent from legend");
    }

    #[test]
    fn handles_empty() {
        let svg = render(&PieDiagram::default(), &Theme::default());
        assert!(svg.starts_with("<svg"));
        // No segments, just outline circle
        assert!(svg.contains("<circle"));
    }
}
