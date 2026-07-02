//! xychart-beta renderer.

use std::fmt::Write as _;

use crate::parse::{XyAxisKind, XyChartDiagram, XySeriesKind};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 40.0;
const TITLE_GAP: f64 = 32.0;
const AXIS_LEFT: f64 = 60.0;
const AXIS_BOTTOM: f64 = 50.0;
const CHART_W: f64 = 600.0;
const CHART_H: f64 = 320.0;

pub(crate) fn render(d: &XyChartDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let width = PAD * 2.0 + AXIS_LEFT + CHART_W + 20.0;
    let height = PAD * 2.0 + title_h + CHART_H + AXIS_BOTTOM + 30.0;
    let chart_left = PAD + AXIS_LEFT;
    let chart_top = PAD + title_h;
    let chart_bottom = chart_top + CHART_H;
    let chart_right = chart_left + CHART_W;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
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
    if let Some(y) = &d.y_axis {
        if let XyAxisKind::Range { min, max } = &y.kind {
            vmin = *min;
            vmax = *max;
        }
    }
    if !vmin.is_finite() {
        vmin = 0.0;
        vmax = 1.0;
    }
    if (vmax - vmin).abs() < 1e-9 {
        vmax = vmin + 1.0;
    }

    let n = d
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0)
        .max(1);
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
    let step = if horiz {
        CHART_H / cats.len() as f64
    } else {
        CHART_W / cats.len() as f64
    };
    // Center coordinate of category `i` along its axis.
    let cat_center = |i: usize| -> f64 {
        if horiz {
            chart_top + (i as f64 + 0.5) * step
        } else {
            chart_left + (i as f64 + 0.5) * step
        }
    };
    // Position of value `v` along the value axis.
    let value_pos = |v: f64| -> f64 {
        let frac = (v - vmin) / (vmax - vmin);
        if horiz {
            chart_left + frac * CHART_W
        } else {
            chart_bottom - frac * CHART_H
        }
    };

    // Value ticks (5 divisions) with grid lines and labels.
    for i in 0..=5 {
        let v = vmin + (vmax - vmin) * (i as f64 / 5.0);
        let p = value_pos(v);
        if horiz {
            svg.line(
                p,
                chart_bottom,
                p,
                chart_bottom + 4.0,
                &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
            );
            svg.line(
                p,
                chart_top,
                p,
                chart_bottom,
                &format!("stroke=\"{fg_muted}\" stroke-width=\"1\" stroke-dasharray=\"2 3\""),
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
            svg.line(
                chart_left,
                p,
                chart_right,
                p,
                &format!("stroke=\"{fg_muted}\" stroke-width=\"1\" stroke-dasharray=\"2 3\""),
            );
            svg.text(
                chart_left - 8.0,
                p + 4.0,
                &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"11\""),
                &fnum(v),
            );
        }
    }

    // Category labels along the category axis.
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

    // Value axis (y-axis) title: rotated on the left when vertical, centered
    // below when horizontal.
    if let Some(t) = d.y_axis.as_ref().and_then(|a| a.title.as_ref()) {
        if horiz {
            svg.text(
                chart_left + CHART_W / 2.0,
                chart_bottom + 38.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                t,
            );
        } else {
            svg.text(
                chart_left - 40.0,
                chart_top + CHART_H / 2.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" transform=\"rotate(-90 {} {})\"",
                    fnum(chart_left - 40.0), fnum(chart_top + CHART_H / 2.0)),
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
                chart_top + CHART_H / 2.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" transform=\"rotate(-90 {} {})\"",
                    fnum(chart_left - 40.0), fnum(chart_top + CHART_H / 2.0)),
                t,
            );
        } else {
            svg.text(
                chart_left + CHART_W / 2.0,
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
        let color = theme.pie_color(si);
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
                    svg.circle(px, py, 3.5, &format!("fill=\"{color}\""));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{XyAxis, XyAxisKind, XySeries, XySeriesKind};

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
                values: vec![40.0, 80.0],
            }],
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
                values: vec![40.0, 80.0],
            }],
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
}
