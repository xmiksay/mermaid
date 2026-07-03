//! Radar (spider) chart renderer.

use std::fmt::Write as _;

use crate::parse::{RadarDiagram, RadarGraticule};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const RADIUS: f64 = 170.0;

pub(crate) fn render(d: &RadarDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let chart_d = RADIUS * 2.0 + 80.0;
    let width = PAD * 2.0 + chart_d + 160.0; // legend area
    let height = PAD * 2.0 + title_h + chart_d;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let cx = PAD + chart_d / 2.0;
    let cy = PAD + title_h + chart_d / 2.0;

    let n = d.axes.len();
    if n < 2 {
        svg.text(
            cx,
            cy,
            &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"13\""),
            "(need >=2 axes)",
        );
        return svg.finish();
    }

    let min = d.min.unwrap_or(0.0);
    let max = d.max.unwrap_or_else(|| {
        d.curves
            .iter()
            .flat_map(|c| c.values.iter().copied())
            .fold(min, f64::max)
    });
    // Guard against a zero/negative span so the scale stays finite.
    let span = (max - min).max(1.0);

    let angle =
        |i: usize| -std::f64::consts::FRAC_PI_2 + (i as f64) * std::f64::consts::TAU / n as f64;

    // Gridlines: `ticks` rings, drawn as concentric circles (default) or as
    // polygon rings following the axis vertices.
    let ticks = d.ticks.unwrap_or(5).max(1);
    for ring in 1..=ticks {
        let r = RADIUS * (ring as f64 / ticks as f64);
        let grid_attrs = format!("fill=\"none\" stroke=\"{fg_muted}\" stroke-width=\"1\"");
        match d.graticule {
            RadarGraticule::Circle => svg.circle(cx, cy, r, &grid_attrs),
            RadarGraticule::Polygon => {
                let mut path = String::new();
                for i in 0..n {
                    let a = angle(i);
                    let x = cx + r * a.cos();
                    let y = cy + r * a.sin();
                    if i == 0 {
                        let _ = write!(path, "M{} {}", fnum(x), fnum(y));
                    } else {
                        let _ = write!(path, "L{} {}", fnum(x), fnum(y));
                    }
                }
                path.push('Z');
                svg.path(&path, &grid_attrs);
            }
        }
    }

    // Spokes + labels.
    for (i, ax) in d.axes.iter().enumerate() {
        let a = angle(i);
        let ex = cx + RADIUS * a.cos();
        let ey = cy + RADIUS * a.sin();
        svg.line(
            cx,
            cy,
            ex,
            ey,
            &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
        );
        let lx = cx + (RADIUS + 14.0) * a.cos();
        let ly = cy + (RADIUS + 14.0) * a.sin() + 4.0;
        svg.text(
            lx,
            ly,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            &ax.label,
        );
    }

    // Curves.
    for (ci, curve) in d.curves.iter().enumerate() {
        if curve.values.is_empty() {
            continue;
        }
        let color = theme.pie_color(ci);
        let mut path = String::new();
        for i in 0..n {
            let v = curve.values.get(i).copied().unwrap_or(min);
            let r = RADIUS * ((v - min) / span).clamp(0.0, 1.0);
            let a = angle(i);
            let x = cx + r * a.cos();
            let y = cy + r * a.sin();
            if i == 0 {
                let _ = write!(path, "M{} {}", fnum(x), fnum(y));
            } else {
                let _ = write!(path, "L{} {}", fnum(x), fnum(y));
            }
        }
        path.push('Z');
        svg.path(
            &path,
            &format!(
                "fill=\"{color}\" fill-opacity=\"0.25\" stroke=\"{color}\" stroke-width=\"2\""
            ),
        );
    }

    // Legend (default on; `showLegend false` suppresses it).
    if !d.show_legend.unwrap_or(true) {
        return svg.finish();
    }
    let lx = PAD + chart_d + 20.0;
    for (ci, curve) in d.curves.iter().enumerate() {
        let color = theme.pie_color(ci);
        let y = PAD + title_h + 20.0 + ci as f64 * 22.0;
        svg.rect(lx, y - 10.0, 14.0, 14.0, &format!("fill=\"{color}\""));
        svg.text(
            lx + 20.0,
            y + 2.0,
            &format!("fill=\"{fg}\" font-size=\"12\""),
            &curve.label,
        );
    }

    svg.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{RadarAxis, RadarCurve};

    #[test]
    fn produces_svg() {
        let d = RadarDiagram {
            title: Some("Skills".into()),
            axes: vec![
                RadarAxis {
                    id: "a".into(),
                    label: "Power".into(),
                },
                RadarAxis {
                    id: "b".into(),
                    label: "Speed".into(),
                },
                RadarAxis {
                    id: "c".into(),
                    label: "Endurance".into(),
                },
            ],
            curves: vec![RadarCurve {
                id: "x".into(),
                label: "A".into(),
                values: vec![3.0, 4.0, 2.0],
            }],
            max: Some(5.0),
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Power<"));
        assert!(svg.contains(">A<"));
    }

    #[test]
    fn default_graticule_is_circles() {
        let d = RadarDiagram {
            axes: vec![
                RadarAxis {
                    id: "a".into(),
                    label: "A".into(),
                },
                RadarAxis {
                    id: "b".into(),
                    label: "B".into(),
                },
                RadarAxis {
                    id: "c".into(),
                    label: "C".into(),
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // Default graticule draws concentric circles, not polygon rings.
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn show_legend_false_omits_swatches() {
        let base = RadarDiagram {
            axes: vec![
                RadarAxis {
                    id: "a".into(),
                    label: "A".into(),
                },
                RadarAxis {
                    id: "b".into(),
                    label: "B".into(),
                },
            ],
            curves: vec![RadarCurve {
                id: "x".into(),
                label: "Legendary".into(),
                values: vec![1.0, 2.0],
            }],
            ..Default::default()
        };
        let with_legend = render(&base, &Theme::default());
        let without = render(
            &RadarDiagram {
                show_legend: Some(false),
                ..base
            },
            &Theme::default(),
        );
        assert!(with_legend.contains(">Legendary<"));
        assert!(!without.contains(">Legendary<"));
    }
}
