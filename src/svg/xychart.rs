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

    // Y ticks (5 divisions).
    for i in 0..=5 {
        let v = vmin + (vmax - vmin) * (i as f64 / 5.0);
        let y = chart_bottom - (i as f64 / 5.0) * CHART_H;
        svg.line(
            chart_left - 4.0,
            y,
            chart_left,
            y,
            &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
        );
        svg.line(
            chart_left,
            y,
            chart_right,
            y,
            &format!("stroke=\"{fg_muted}\" stroke-width=\"1\" stroke-dasharray=\"2 3\""),
        );
        svg.text(
            chart_left - 8.0,
            y + 4.0,
            &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"11\""),
            &fnum(v),
        );
    }

    if let Some(y) = &d.y_axis {
        if let Some(t) = &y.title {
            // Vertical label.
            svg.text(
                chart_left - 40.0,
                chart_top + CHART_H / 2.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" transform=\"rotate(-90 {} {})\"",
                    fnum(chart_left - 40.0), fnum(chart_top + CHART_H / 2.0)),
                t,
            );
        }
    }

    // X labels.
    let step = CHART_W / cats.len() as f64;
    for (i, c) in cats.iter().enumerate() {
        let x = chart_left + (i as f64 + 0.5) * step;
        svg.text(
            x,
            chart_bottom + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
            c,
        );
    }

    if let Some(x) = &d.x_axis {
        if let Some(t) = &x.title {
            svg.text(
                chart_left + CHART_W / 2.0,
                chart_bottom + 38.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
                t,
            );
        }
    }

    let scale_y = |v: f64| chart_bottom - (v - vmin) / (vmax - vmin) * CHART_H;

    // Series.
    let bar_count = d
        .series
        .iter()
        .filter(|s| s.kind == XySeriesKind::Bar)
        .count();
    let mut bar_idx = 0;
    for (si, s) in d.series.iter().enumerate() {
        let color = theme.pie_color(si);
        match s.kind {
            XySeriesKind::Bar => {
                let bar_w = (step * 0.7) / bar_count.max(1) as f64;
                for (i, v) in s.values.iter().enumerate() {
                    let cx = chart_left + (i as f64 + 0.5) * step;
                    let x = cx - (bar_w * bar_count as f64) / 2.0 + bar_idx as f64 * bar_w;
                    let y = scale_y(*v);
                    let h = (chart_bottom - y).max(0.0);
                    svg.rect(x, y, bar_w, h, &format!("fill=\"{color}\""));
                }
                bar_idx += 1;
            }
            XySeriesKind::Line => {
                let mut path = String::new();
                for (i, v) in s.values.iter().enumerate() {
                    let cx = chart_left + (i as f64 + 0.5) * step;
                    let y = scale_y(*v);
                    if i == 0 {
                        let _ = write!(path, "M{} {}", fnum(cx), fnum(y));
                    } else {
                        let _ = write!(path, "L{} {}", fnum(cx), fnum(y));
                    }
                    svg.circle(cx, y, 3.5, &format!("fill=\"{color}\""));
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
}
