//! Radar (spider) chart renderer.

use std::fmt::Write as _;

use crate::parse::{RadarDiagram, RadarGraticule};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const RADIUS: f64 = 190.0;
/// Room reserved around the rings for the axis labels (each side).
const LABEL_PAD: f64 = 40.0;
/// Width of the legend column to the right of the chart.
const LEGEND_W: f64 = 160.0;
/// Upstream `radar.curveTension` default for the closed cardinal spline.
const DEFAULT_TENSION: f64 = 0.17;
/// Half-length of the dark tick capping each spoke at the outer ring.
const TICK_LEN: f64 = 4.0;

pub(crate) fn render(d: &RadarDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };

    // Margins: `config.radar.margin*` override the symmetric `PAD` default.
    let m_top = d.margin_top.unwrap_or(PAD);
    let m_bottom = d.margin_bottom.unwrap_or(PAD);
    let m_left = d.margin_left.unwrap_or(PAD);
    let m_right = d.margin_right.unwrap_or(PAD);

    // Overall SVG size — `config.radar.width/height` override the derived
    // defaults (680×520 / +title, sized so the disc fills the canvas like
    // upstream).
    let default_chart_d = RADIUS * 2.0 + LABEL_PAD * 2.0;
    let default_width = m_left + m_right + default_chart_d + LEGEND_W;
    let default_height = m_top + m_bottom + title_h + default_chart_d;
    let width = d.width.unwrap_or(default_width).max(1.0);
    let height = d.height.unwrap_or(default_height).max(1.0);

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            m_top + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\""),
            t,
        );
    }

    // Chart area sits left of the legend column; the radar is centred in it.
    let legend_w = if d.show_legend.unwrap_or(true) {
        LEGEND_W
    } else {
        0.0
    };
    let avail_w = (width - m_left - m_right - legend_w).max(2.0);
    let avail_h = (height - m_top - m_bottom - title_h).max(2.0);
    let cx = m_left + avail_w / 2.0;
    let cy = m_top + title_h + avail_h / 2.0;
    // Graticule/spoke radius fits the available area; the axis-value scale
    // (curve reach) additionally multiplies by `axisScaleFactor`.
    let radius = (avail_w.min(avail_h) / 2.0 - LABEL_PAD).max(10.0);
    let axis_scale = d.axis_scale_factor.unwrap_or(1.0).max(0.0);
    let plot_r = radius * axis_scale;

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

    // Graticule: a filled light-gray disc behind `ticks` fainter rings —
    // matching upstream's solid disc + subtle ring outlines. Rings are drawn as
    // concentric circles (default) or polygon rings following the axis vertices.
    let ticks = d.ticks.unwrap_or(5).max(1);
    let disc_attrs = format!("fill=\"{fg_muted}\" fill-opacity=\"0.12\" stroke=\"none\"");
    draw_ring(&mut svg, d.graticule, cx, cy, radius, n, &disc_attrs);
    for ring in 1..=ticks {
        let r = radius * (ring as f64 / ticks as f64);
        let grid_attrs = format!(
            "fill=\"none\" stroke=\"{fg_muted}\" stroke-opacity=\"0.35\" stroke-width=\"1\""
        );
        draw_ring(&mut svg, d.graticule, cx, cy, r, n, &grid_attrs);
    }

    // Spokes + labels; each spoke is capped with a short dark tick perpendicular
    // to it at the outer ring, as upstream draws.
    for (i, ax) in d.axes.iter().enumerate() {
        let a = angle(i);
        let ex = cx + radius * a.cos();
        let ey = cy + radius * a.sin();
        svg.line(
            cx,
            cy,
            ex,
            ey,
            &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
        );
        // Tick perpendicular to the spoke (`(-sin a, cos a)`) at its outer end.
        let (tx, ty) = (-a.sin(), a.cos());
        svg.line(
            ex - TICK_LEN * tx,
            ey - TICK_LEN * ty,
            ex + TICK_LEN * tx,
            ey + TICK_LEN * ty,
            &format!("stroke=\"{fg}\" stroke-width=\"1.5\""),
        );
        // Anchor labels toward their outer side (start on the right, end on the
        // left, middle near the vertical) so long words like "Intelligence" grow
        // away from the disc instead of overlapping its edge (#330).
        let ux = a.cos();
        let anchor = if ux > 0.3 {
            "start"
        } else if ux < -0.3 {
            "end"
        } else {
            "middle"
        };
        let lx = cx + (radius + 14.0) * ux;
        let ly = cy + (radius + 14.0) * a.sin() + 4.0;
        svg.text(
            lx,
            ly,
            &format!("text-anchor=\"{anchor}\" fill=\"{fg}\" font-size=\"12\""),
            &ax.label,
        );
    }

    // Curves. Upstream draws a closed rounded cardinal (Catmull-Rom) spline
    // for the default circle graticule and straight segments for polygon.
    let tension = d.curve_tension.unwrap_or(DEFAULT_TENSION);
    for (ci, curve) in d.curves.iter().enumerate() {
        if curve.values.is_empty() {
            continue;
        }
        let color = theme.cscale_color(ci);
        let pts: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let v = curve.values.get(i).copied().unwrap_or(min);
                let r = plot_r * ((v - min) / span).clamp(0.0, 1.0);
                let a = angle(i);
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        let path = match d.graticule {
            RadarGraticule::Circle => cardinal_closed_path(&pts, tension),
            RadarGraticule::Polygon => straight_closed_path(&pts),
        };
        // Upstream `radar.curveOpacity` (0.5) — saturated enough that the faint
        // graticule rings underneath do not read through the fill (#330).
        svg.path(
            &path,
            &format!("fill=\"{color}\" fill-opacity=\"0.5\" stroke=\"{color}\" stroke-width=\"2\""),
        );
    }

    // Legend (default on; `showLegend false` suppresses it).
    if !d.show_legend.unwrap_or(true) {
        return svg.finish();
    }
    let lx = m_left + avail_w + 20.0;
    for (ci, curve) in d.curves.iter().enumerate() {
        let color = theme.cscale_color(ci);
        let y = m_top + title_h + 20.0 + ci as f64 * 22.0;
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

/// Draw one graticule outline at radius `r` — a circle (default) or an `n`-gon
/// following the axis vertices — with the given presentation attributes.
fn draw_ring(
    svg: &mut SvgBuilder,
    graticule: RadarGraticule,
    cx: f64,
    cy: f64,
    r: f64,
    n: usize,
    attrs: &str,
) {
    match graticule {
        RadarGraticule::Circle => svg.circle(cx, cy, r, attrs),
        RadarGraticule::Polygon => {
            let mut path = String::new();
            for i in 0..n {
                let a =
                    -std::f64::consts::FRAC_PI_2 + (i as f64) * std::f64::consts::TAU / n as f64;
                let x = cx + r * a.cos();
                let y = cy + r * a.sin();
                if i == 0 {
                    let _ = write!(path, "M{} {}", fnum(x), fnum(y));
                } else {
                    let _ = write!(path, "L{} {}", fnum(x), fnum(y));
                }
            }
            path.push('Z');
            svg.path(&path, attrs);
        }
    }
}

/// Closed polyline through `pts` (`M … L … Z`), used for the polygon graticule.
fn straight_closed_path(pts: &[(f64, f64)]) -> String {
    let mut path = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            let _ = write!(path, "M{} {}", fnum(*x), fnum(*y));
        } else {
            let _ = write!(path, "L{} {}", fnum(*x), fnum(*y));
        }
    }
    path.push('Z');
    path
}

/// Closed cardinal (Catmull-Rom) spline through `pts` — d3's
/// `curveCardinalClosed.tension(t)`: each cubic segment uses control points
/// `p1 + k·(p2 − p0)` and `p2 + k·(p1 − p3)` with `k = (1 − t)/6` and indices
/// wrapping around the ring.
fn cardinal_closed_path(pts: &[(f64, f64)], tension: f64) -> String {
    let n = pts.len();
    if n < 3 {
        return straight_closed_path(pts);
    }
    let k = (1.0 - tension) / 6.0;
    let at = |i: isize| pts[i.rem_euclid(n as isize) as usize];
    let mut path = String::new();
    let (sx, sy) = pts[0];
    let _ = write!(path, "M{} {}", fnum(sx), fnum(sy));
    for i in 0..n as isize {
        let (x0, y0) = at(i - 1);
        let (x1, y1) = at(i);
        let (x2, y2) = at(i + 1);
        let (x3, y3) = at(i + 2);
        let c1x = x1 + k * (x2 - x0);
        let c1y = y1 + k * (y2 - y0);
        let c2x = x2 + k * (x1 - x3);
        let c2y = y2 + k * (y1 - y3);
        let _ = write!(
            path,
            "C{} {} {} {} {} {}",
            fnum(c1x),
            fnum(c1y),
            fnum(c2x),
            fnum(c2y),
            fnum(x2),
            fnum(y2)
        );
    }
    path.push('Z');
    path
}

#[cfg(test)]
mod tests;
