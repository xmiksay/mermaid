//! xychart-beta renderer.

use std::fmt::Write as _;

use crate::parse::{XyAxisKind, XyChartDiagram, XySeriesKind};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

mod legend;
#[cfg(test)]
mod tests;
mod ticks;

use legend::{draw_legend, draw_point_label};
use ticks::nice_ticks;

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
