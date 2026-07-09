//! xychart-beta renderer.

use std::fmt::Write as _;

use crate::parse::{XyAxisKind, XyChartDiagram, XySeriesKind};

use super::builder::{fnum, SvgBuilder};
use super::metrics::text_width;
use super::theme::Theme;

const PAD: f64 = 40.0;
const TITLE_GAP: f64 = 32.0;
const AXIS_LEFT: f64 = 60.0;
const AXIS_BOTTOM: f64 = 50.0;
const CHART_W: f64 = 600.0;
const CHART_H: f64 = 320.0;

pub(crate) fn render(d: &XyChartDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    let chart_w = d.width.unwrap_or(CHART_W);
    let chart_h = d.height.unwrap_or(CHART_H);

    // Legend entries: one per titled series (upstream `showLegend`, default on).
    let show_legend = d.show_legend.unwrap_or(true);
    let legend: Vec<(usize, &str)> = if show_legend {
        d.series
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.title.as_deref().map(|t| (i, t)))
            .collect()
    } else {
        Vec::new()
    };

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let legend_h = if legend.is_empty() { 0.0 } else { 24.0 };
    let width = PAD * 2.0 + AXIS_LEFT + chart_w + 20.0;
    let height = PAD * 2.0 + title_h + legend_h + chart_h + AXIS_BOTTOM + 30.0;
    let chart_left = PAD + AXIS_LEFT;
    let chart_top = PAD + title_h + legend_h;
    let chart_bottom = chart_top + chart_h;
    let chart_right = chart_left + chart_w;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\""),
            t,
        );
    }

    let color_at = |i: usize| -> String {
        if d.plot_color_palette.is_empty() {
            theme.xychart_color(i).to_string()
        } else {
            d.plot_color_palette[i % d.plot_color_palette.len()].clone()
        }
    };

    if !legend.is_empty() {
        draw_legend(&mut svg, &legend, &color_at, width, PAD + title_h, fg);
    }

    // Determine value range.
    let mut vmin = f64::INFINITY;
    let mut vmax = f64::NEG_INFINITY;
    for s in &d.series {
        for v in &s.values {
            vmin = vmin.min(*v);
            vmax = vmax.max(*v);
        }
    }
    let y_explicit = matches!(
        d.y_axis.as_ref().map(|a| &a.kind),
        Some(XyAxisKind::Range { .. })
    );
    if let Some(XyAxisKind::Range { min, max }) = d.y_axis.as_ref().map(|a| &a.kind) {
        vmin = *min;
        vmax = *max;
    }
    // Bar charts baseline at zero unless an explicit y-range says otherwise —
    // an auto range spanning only the data would start bars mid-axis.
    if !y_explicit && vmin.is_finite() && d.series.iter().any(|s| s.kind == XySeriesKind::Bar) {
        vmin = vmin.min(0.0);
        vmax = vmax.max(0.0);
    }
    if !vmin.is_finite() {
        vmin = 0.0;
        vmax = 1.0;
    }
    if (vmax - vmin).abs() < 1e-9 {
        vmax = vmin + 1.0;
    }
    // Round the value domain to "nice" bounds and derive round tick values so
    // the axis reads 4000, 4500, … rather than the raw 1/5-range divisions.
    let (nice_min, nice_max, value_ticks) = nice_ticks(vmin, vmax);
    vmin = nice_min;
    vmax = nice_max;

    let n = d
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0)
        .max(1);
    // A numeric x-axis positions each point by its x value (its 1-based index)
    // scaled through the range; otherwise points sit at category centers.
    let x_range = match d.x_axis.as_ref().map(|a| &a.kind) {
        Some(XyAxisKind::Range { min, max }) if (max - min).abs() > 1e-9 => Some((*min, *max)),
        _ => None,
    };
    let cats: Vec<String> = match d.x_axis.as_ref().map(|a| &a.kind) {
        Some(XyAxisKind::Categories(c)) if !c.is_empty() => c.clone(),
        _ => (1..=n).map(|i| i.to_string()).collect(),
    };

    // Axes.
    svg.line(
        chart_left,
        chart_top,
        chart_left,
        chart_bottom,
        &format!("stroke=\"{fg}\" stroke-width=\"1.5\""),
    );
    svg.line(
        chart_left,
        chart_bottom,
        chart_right,
        chart_bottom,
        &format!("stroke=\"{fg}\" stroke-width=\"1.5\""),
    );

    // When horizontal, the category axis (x-axis) runs down the left and the
    // value axis (y-axis) runs along the bottom; bars grow rightward.
    let horiz = d.horizontal;

    // Category tick spacing along the category axis (bottom when vertical,
    // left when horizontal).
    let cat_axis_len = if horiz { chart_h } else { chart_w };
    // For a numeric axis one step spans a single x unit; for categories it is
    // the per-category slot width used for bar thickness.
    let step = match x_range {
        Some((xmin, xmax)) => cat_axis_len / (xmax - xmin),
        None => cat_axis_len / cats.len() as f64,
    };
    let cat_origin = if horiz { chart_top } else { chart_left };
    // Center coordinate of point `i` along the category axis.
    let cat_center = |i: usize| -> f64 {
        match x_range {
            // Explicit x values are the point's 1-based index.
            Some((xmin, xmax)) => {
                let frac = ((i + 1) as f64 - xmin) / (xmax - xmin);
                cat_origin + frac * cat_axis_len
            }
            None => cat_origin + (i as f64 + 0.5) * step,
        }
    };
    // Position of value `v` along the value axis.
    let value_pos = |v: f64| -> f64 {
        let frac = (v - vmin) / (vmax - vmin);
        if horiz {
            chart_left + frac * chart_w
        } else {
            chart_bottom - frac * chart_h
        }
    };

    // Value ticks (nice round values): a short tick mark and a label. Upstream
    // draws no gridlines across the plot area, so we don't either (#319).
    for &v in &value_ticks {
        let p = value_pos(v);
        if horiz {
            svg.line(
                p,
                chart_bottom,
                p,
                chart_bottom + 4.0,
                &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
            );
            svg.text(
                p,
                chart_bottom + 18.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                &fnum(v),
            );
        } else {
            svg.line(
                chart_left - 4.0,
                p,
                chart_left,
                p,
                &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
            );
            svg.text(
                chart_left - 8.0,
                p + 4.0,
                &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"11\""),
                &fnum(v),
            );
        }
    }

    // Category axis labels: numeric ticks (5 divisions) for a range axis, or
    // one label per category otherwise.
    if let Some((xmin, xmax)) = x_range {
        for i in 0..=5 {
            let xv = xmin + (xmax - xmin) * (i as f64 / 5.0);
            let frac = (xv - xmin) / (xmax - xmin);
            let p = cat_origin + frac * cat_axis_len;
            if horiz {
                svg.line(
                    chart_left - 4.0,
                    p,
                    chart_left,
                    p,
                    &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
                );
                svg.text(
                    chart_left - 8.0,
                    p + 4.0,
                    &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"11\""),
                    &fnum(xv),
                );
            } else {
                svg.line(
                    p,
                    chart_bottom,
                    p,
                    chart_bottom + 4.0,
                    &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
                );
                svg.text(
                    p,
                    chart_bottom + 18.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                    &fnum(xv),
                );
            }
        }
    } else {
        for (i, c) in cats.iter().enumerate() {
            let p = cat_center(i);
            if horiz {
                svg.text(
                    chart_left - 8.0,
                    p + 4.0,
                    &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"11\""),
                    c,
                );
            } else {
                svg.text(
                    p,
                    chart_bottom + 18.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                    c,
                );
            }
        }
    }

    // Value axis (y-axis) title: rotated on the left when vertical, centered
    // below when horizontal.
    if let Some(t) = d.y_axis.as_ref().and_then(|a| a.title.as_ref()) {
        if horiz {
            svg.text(
                chart_left + chart_w / 2.0,
                chart_bottom + 38.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                t,
            );
        } else {
            svg.text(
                chart_left - 40.0,
                chart_top + chart_h / 2.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" transform=\"rotate(-90 {} {})\"",
                    fnum(chart_left - 40.0), fnum(chart_top + chart_h / 2.0)),
                t,
            );
        }
    }

    // Category axis (x-axis) title: centered below when vertical, rotated on
    // the left when horizontal.
    if let Some(t) = d.x_axis.as_ref().and_then(|a| a.title.as_ref()) {
        if horiz {
            svg.text(
                chart_left - 40.0,
                chart_top + chart_h / 2.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" transform=\"rotate(-90 {} {})\"",
                    fnum(chart_left - 40.0), fnum(chart_top + chart_h / 2.0)),
                t,
            );
        } else {
            svg.text(
                chart_left + chart_w / 2.0,
                chart_bottom + 38.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                t,
            );
        }
    }

    // Series.
    let bar_count = d
        .series
        .iter()
        .filter(|s| s.kind == XySeriesKind::Bar)
        .count();
    let mut bar_idx = 0;
    for (si, s) in d.series.iter().enumerate() {
        let color = color_at(si);
        let label_of = |i: usize| s.labels.get(i).and_then(|l| l.as_deref());
        let bar_w = (step * 0.7) / bar_count.max(1) as f64;
        match s.kind {
            XySeriesKind::Bar => {
                for (i, v) in s.values.iter().enumerate() {
                    let center = cat_center(i);
                    let off = center - (bar_w * bar_count as f64) / 2.0 + bar_idx as f64 * bar_w;
                    let p = value_pos(*v);
                    if horiz {
                        let w = (p - chart_left).max(0.0);
                        svg.rect(chart_left, off, w, bar_w, &format!("fill=\"{color}\""));
                    } else {
                        let h = (chart_bottom - p).max(0.0);
                        svg.rect(off, p, bar_w, h, &format!("fill=\"{color}\""));
                    }
                    if let Some(label) = label_of(i) {
                        let (lx, ly) = if horiz {
                            (p + 4.0, off + bar_w / 2.0 + 4.0)
                        } else {
                            (center, p - 6.0)
                        };
                        draw_point_label(&mut svg, lx, ly, horiz, fg, label);
                    }
                }
                bar_idx += 1;
            }
            XySeriesKind::Line => {
                let mut path = String::new();
                for (i, v) in s.values.iter().enumerate() {
                    let center = cat_center(i);
                    let p = value_pos(*v);
                    let (px, py) = if horiz { (p, center) } else { (center, p) };
                    if i == 0 {
                        let _ = write!(path, "M{} {}", fnum(px), fnum(py));
                    } else {
                        let _ = write!(path, "L{} {}", fnum(px), fnum(py));
                    }
                    // Upstream draws no point markers on the line (#319).
                    if let Some(label) = label_of(i) {
                        draw_point_label(&mut svg, px, py - 8.0, false, fg, label);
                    }
                }
                svg.path(
                    &path,
                    &format!("fill=\"none\" stroke=\"{color}\" stroke-width=\"2\""),
                );
            }
        }
    }

    svg.finish()
}

/// Draw a centered legend row of colored swatches + series titles just above
/// the plot, starting at `top`.
fn draw_legend(
    svg: &mut SvgBuilder,
    entries: &[(usize, &str)],
    color_at: &dyn Fn(usize) -> String,
    width: f64,
    top: f64,
    fg: &str,
) {
    const SWATCH: f64 = 12.0;
    const GAP: f64 = 6.0;
    const ITEM_GAP: f64 = 18.0;
    let entry_w = |t: &str| SWATCH + GAP + text_width(t, 7.0, 12.0);
    let total: f64 = entries.iter().map(|(_, t)| entry_w(t)).sum::<f64>()
        + ITEM_GAP * (entries.len().saturating_sub(1)) as f64;
    let mut x = (width - total) / 2.0;
    for (i, t) in entries {
        svg.rect(
            x,
            top,
            SWATCH,
            SWATCH,
            &format!("fill=\"{}\"", color_at(*i)),
        );
        svg.text(
            x + SWATCH + GAP,
            top + SWATCH - 2.0,
            &format!("text-anchor=\"start\" fill=\"{fg}\" font-size=\"12\""),
            t,
        );
        x += entry_w(t) + ITEM_GAP;
    }
}

/// Draw a per-point data label. Horizontal charts anchor it to the start
/// (right of the point); vertical charts center it above the point.
fn draw_point_label(svg: &mut SvgBuilder, x: f64, y: f64, horiz: bool, fg: &str, label: &str) {
    let anchor = if horiz { "start" } else { "middle" };
    svg.text(
        x,
        y,
        &format!("text-anchor=\"{anchor}\" fill=\"{fg}\" font-size=\"10\""),
        label,
    );
}

/// Round a value domain to "nice" bounds and enumerate round tick values,
/// mirroring d3's `ticks()`/`nice()`: pick a step of 1/2/5 × 10^k so ~10 ticks
/// fit the span, then extend the domain out to the nearest step multiples.
/// Returns `(nice_min, nice_max, ticks)`.
fn nice_ticks(vmin: f64, vmax: f64) -> (f64, f64, Vec<f64>) {
    const TARGET: f64 = 10.0;
    let step = tick_step(vmax - vmin, TARGET);
    let lo = (vmin / step).floor() * step;
    let hi = (vmax / step).ceil() * step;
    let count = ((hi - lo) / step).round().max(1.0) as usize;
    let ticks = (0..=count).map(|i| lo + i as f64 * step).collect();
    (lo, hi, ticks)
}

/// The "nice" step size (1/2/5 × 10^k) that fits roughly `count` ticks across
/// `span`, matching d3's `tickStep`.
fn tick_step(span: f64, count: f64) -> f64 {
    let step0 = span.abs() / count.max(1.0);
    let mag = 10f64.powf(step0.log10().floor());
    let error = step0 / mag;
    // d3's thresholds: √50, √10, √2.
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    factor * mag
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{XyAxis, XyAxisKind, XySeries, XySeriesKind};

    #[test]
    fn nice_ticks_produce_round_steps() {
        // The 4000–11000 sample range: d3-style nice steps of 500.
        let (lo, hi, ticks) = nice_ticks(4000.0, 11000.0);
        assert_eq!(lo, 4000.0);
        assert_eq!(hi, 11000.0);
        assert_eq!(
            ticks,
            vec![
                4000.0, 4500.0, 5000.0, 5500.0, 6000.0, 6500.0, 7000.0, 7500.0, 8000.0, 8500.0,
                9000.0, 9500.0, 10000.0, 10500.0, 11000.0
            ]
        );

        // A domain that needs extending: 3..97 → nice 0..100 step 10.
        let (lo, hi, ticks) = nice_ticks(3.0, 97.0);
        assert_eq!((lo, hi), (0.0, 100.0));
        assert_eq!(ticks.first(), Some(&0.0));
        assert_eq!(ticks.last(), Some(&100.0));
        assert!(ticks.windows(2).all(|w| (w[1] - w[0] - 10.0).abs() < 1e-9));

        // A small span picks a fractional step.
        let (_, _, ticks) = nice_ticks(0.0, 1.0);
        assert!(ticks.windows(2).all(|w| (w[1] - w[0] - 0.1).abs() < 1e-9));
    }

    #[test]
    fn renders_nice_value_ticks_in_svg() {
        let d = XyChartDiagram {
            y_axis: Some(XyAxis {
                title: None,
                kind: XyAxisKind::Range {
                    min: 4000.0,
                    max: 11000.0,
                },
            }),
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![5000.0, 9500.0],
                labels: Vec::new(),
            }],
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        // Round tick labels, not the old 1/5-range divisions (e.g. 5400).
        assert!(svg.contains(">4500<"));
        assert!(svg.contains(">10500<"));
        assert!(!svg.contains(">5400<"));
    }

    #[test]
    fn title_uses_regular_weight() {
        // Upstream renders the chart title at regular weight, not bold (#332).
        let d = XyChartDiagram {
            title: Some("Sales".into()),
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("font-size=\"18\">Sales</text>"));
        assert!(!svg.contains("font-weight=\"bold\">Sales<"));
    }

    #[test]
    fn produces_svg() {
        let d = XyChartDiagram {
            horizontal: false,
            title: Some("Sales".into()),
            x_axis: Some(XyAxis {
                title: None,
                kind: XyAxisKind::Categories(vec!["Jan".into(), "Feb".into()]),
            }),
            y_axis: Some(XyAxis {
                title: Some("$".into()),
                kind: XyAxisKind::Range {
                    min: 0.0,
                    max: 100.0,
                },
            }),
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            }],
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Sales<"));
        assert!(svg.contains(">Jan<"));
    }

    // Extract the `width`/`height` of every <rect> bar (skip other rects).
    fn bar_dims(svg: &str) -> Vec<(f64, f64)> {
        svg.match_indices("<rect ")
            .filter_map(|(i, _)| {
                let tag = &svg[i..svg[i..].find("/>").map(|e| i + e).unwrap_or(svg.len())];
                let attr = |name: &str| -> Option<f64> {
                    let key = format!("{name}=\"");
                    let start = tag.find(&key)? + key.len();
                    let end = tag[start..].find('"')? + start;
                    tag[start..end].parse().ok()
                };
                Some((attr("width")?, attr("height")?))
            })
            .collect()
    }

    #[test]
    fn horizontal_bars_grow_in_width() {
        let make = |horizontal: bool| XyChartDiagram {
            horizontal,
            title: None,
            x_axis: Some(XyAxis {
                title: None,
                kind: XyAxisKind::Categories(vec!["A".into(), "B".into()]),
            }),
            y_axis: Some(XyAxis {
                title: None,
                kind: XyAxisKind::Range {
                    min: 0.0,
                    max: 100.0,
                },
            }),
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            }],
            ..XyChartDiagram::default()
        };

        // Horizontal: value maps to bar width; the 80 bar is wider than 40,
        // and both share a constant height (the bar thickness).
        let h = bar_dims(&render(&make(true), &Theme::default()));
        assert_eq!(h.len(), 2);
        assert!(h[1].0 > h[0].0, "width should grow with value: {h:?}");
        assert!((h[0].1 - h[1].1).abs() < 1e-9, "bar height constant: {h:?}");

        // Vertical: the same values map to height instead; widths are equal.
        let v = bar_dims(&render(&make(false), &Theme::default()));
        assert_eq!(v.len(), 2);
        assert!(v[1].1 > v[0].1, "height should grow with value: {v:?}");
        assert!((v[0].0 - v[1].0).abs() < 1e-9, "bar width constant: {v:?}");
    }

    // Extract the `x` of every <rect> bar in document order.
    fn bar_xs(svg: &str) -> Vec<f64> {
        svg.match_indices("<rect ")
            .filter_map(|(i, _)| {
                let tag = &svg[i..svg[i..].find("/>").map(|e| i + e).unwrap_or(svg.len())];
                let key = "x=\"";
                let start = tag.find(key)? + key.len();
                let end = tag[start..].find('"')? + start;
                tag[start..end].parse().ok()
            })
            .collect()
    }

    #[test]
    fn bar_auto_range_baselines_at_zero() {
        // No y-axis: an auto range with a bar series must include zero, so the
        // 40 and 80 bars stand in a 1:2 height ratio (not the 0:full a
        // data-only 40..80 range would give).
        let d = XyChartDiagram {
            horizontal: false,
            title: None,
            x_axis: None,
            y_axis: None,
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            }],
            ..XyChartDiagram::default()
        };
        let h = bar_dims(&render(&d, &Theme::default()));
        assert_eq!(h.len(), 2);
        assert!(h[0].1 > 1.0, "smaller bar must be zero-baselined: {h:?}");
        assert!((h[1].1 / h[0].1 - 2.0).abs() < 1e-6, "1:2 ratio: {h:?}");
    }

    #[test]
    fn numeric_x_axis_positions_and_ticks() {
        let d = XyChartDiagram {
            horizontal: false,
            title: None,
            x_axis: Some(XyAxis {
                title: None,
                kind: XyAxisKind::Range {
                    min: 0.0,
                    max: 10.0,
                },
            }),
            y_axis: None,
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![3.0, 6.0],
                labels: Vec::new(),
            }],
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        // Numeric ticks are emitted along the x-axis (the range max is a tick).
        assert!(svg.contains(">10<"), "numeric x tick missing");
        // Points map through the range, not to category centers: the two bars
        // (x = 1 and x = 2 over 0..10) sit in the left tenth of the chart, one
        // step apart — not the ~half-chart spacing category centers would give.
        let xs = bar_xs(&svg);
        assert_eq!(xs.len(), 2);
        let gap = xs[1] - xs[0];
        assert!(
            (gap - CHART_W / 10.0).abs() < 1e-6,
            "points one x-unit apart: {xs:?}"
        );
    }

    #[test]
    fn renders_legend_and_palette() {
        let d = XyChartDiagram {
            series: vec![
                XySeries {
                    kind: XySeriesKind::Bar,
                    title: Some("Revenue".into()),
                    values: vec![40.0, 80.0],
                    labels: Vec::new(),
                },
                XySeries {
                    kind: XySeriesKind::Line,
                    title: Some("Trend".into()),
                    values: vec![40.0, 80.0],
                    labels: Vec::new(),
                },
            ],
            plot_color_palette: vec!["#111111".into(), "#222222".into()],
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        // Legend shows both series titles and the palette drives the fills.
        assert!(svg.contains(">Revenue<"));
        assert!(svg.contains(">Trend<"));
        assert!(svg.contains("fill=\"#111111\""));
        assert!(svg.contains("fill=\"#222222\""));
    }

    #[test]
    fn default_palette_and_no_extra_elements() {
        // Upstream: pale-lavender bars, dark gray-blue line; no dotted gridlines
        // across the plot and no circular point markers (#319).
        let d = XyChartDiagram {
            series: vec![
                XySeries {
                    kind: XySeriesKind::Bar,
                    title: None,
                    values: vec![40.0, 80.0],
                    labels: Vec::new(),
                },
                XySeries {
                    kind: XySeriesKind::Line,
                    title: None,
                    values: vec![40.0, 80.0],
                    labels: Vec::new(),
                },
            ],
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#ECECFF\""), "bar uses pale lavender");
        assert!(
            svg.contains("stroke=\"#8493A6\""),
            "line uses dark gray-blue"
        );
        assert!(!svg.contains("stroke-dasharray"), "no dotted gridlines");
        assert!(!svg.contains("<circle"), "no point markers");
    }

    #[test]
    fn hides_legend_when_disabled() {
        let d = XyChartDiagram {
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: Some("Revenue".into()),
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            }],
            show_legend: Some(false),
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(!svg.contains(">Revenue<"));
    }

    #[test]
    fn renders_point_labels() {
        let d = XyChartDiagram {
            series: vec![XySeries {
                kind: XySeriesKind::Line,
                title: None,
                values: vec![40.0, 80.0],
                labels: vec![Some("low".into()), Some("high".into())],
            }],
            ..XyChartDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">low<"));
        assert!(svg.contains(">high<"));
    }

    #[test]
    fn width_height_config_resizes_plot() {
        let base = XyChartDiagram {
            series: vec![XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            }],
            ..XyChartDiagram::default()
        };
        let wide = XyChartDiagram {
            width: Some(CHART_W * 2.0),
            ..base.clone()
        };
        let root_width = |svg: &str| -> f64 {
            let key = "viewBox=\"0 0 ";
            let start = svg.find(key).unwrap() + key.len();
            let rest = &svg[start..];
            rest[..rest.find(' ').unwrap()].parse().unwrap()
        };
        assert!(
            root_width(&render(&wide, &Theme::default()))
                > root_width(&render(&base, &Theme::default()))
        );
    }
}
