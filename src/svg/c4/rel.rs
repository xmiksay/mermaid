//! C4 relation drawing: point-to-point connectors clipped to each node, drawn
//! as quadratic Bézier curves with an arrow head and an optional `[techn]` tag.

use std::collections::HashMap;

use crate::parse::{C4RelStyle, C4Relation};

use super::super::builder::{fnum, SvgBuilder};
use super::super::geometry::{clip_rect, polyline_midpoint};
use super::super::label::edge_label_bg;
use super::super::metrics::text_width;
use super::super::theme::Theme;
use super::C4_LINE;

pub(super) fn draw_rel(
    r: &C4Relation,
    ov: Option<&C4RelStyle>,
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let fg: &str = ov
        .and_then(|s| s.text_color.as_deref())
        .unwrap_or(&theme.fg);
    let fg_muted = &theme.fg_muted;
    let stroke: &str = ov.and_then(|s| s.line_color.as_deref()).unwrap_or(C4_LINE);

    let Some(&(ax, ay, aw, ah)) = pos.get(&r.from) else {
        return;
    };
    let Some(&(bx, by, bw, bh)) = pos.get(&r.to) else {
        return;
    };

    let (sx, sy) = (ax + aw / 2.0, ay + ah / 2.0);
    let (tx, ty) = (bx + bw / 2.0, by + bh / 2.0);

    // Point-to-point line, clipped to each node's rectangle.
    let p_first = clip_rect((tx, ty), (sx, sy), (aw, ah));
    let p_last = clip_rect((sx, sy), (tx, ty), (bw, bh));
    let clipped = vec![p_first, p_last];

    let markers = if r.bidirectional {
        "marker-start=\"url(#c4-arrow)\" marker-end=\"url(#c4-arrow)\""
    } else {
        "marker-end=\"url(#c4-arrow)\""
    };

    // Upstream draws relations as a quadratic Bézier through the routed midpoint.
    let path = quad_path(&clipped);
    svg.path(
        &path,
        &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\" {markers}"),
    );

    let label = &r.label;
    let tech = r.technology.as_deref();
    if label.is_empty() && tech.is_none() {
        return;
    }
    let (mut mx, mut my) = polyline_midpoint(&clipped);
    if let Some(s) = ov {
        mx += s.offset_x.unwrap_or(0.0);
        my += s.offset_y.unwrap_or(0.0);
    }
    // Opaque background behind each label line so the relation label stays
    // legible where it crosses geometry (upstream `edgeLabelBackground`, #260).
    if let Some(t) = tech {
        let label = truncate(label, 36);
        let tech = format!("[{}]", truncate(t, 30));
        let lw = text_width(&label, 8.0, 10.0) + 6.0;
        let tw = text_width(&tech, 8.0, 9.0) + 6.0;
        edge_label_bg(svg, mx, my - 4.0, lw, 13.0, theme);
        edge_label_bg(svg, mx, my + 9.0, tw, 12.0, theme);
        svg.text(
            mx,
            my - 1.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &label,
        );
        svg.text(
            mx,
            my + 12.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\" font-style=\"italic\""
            ),
            &tech,
        );
    } else {
        let label = truncate(label, 36);
        let lw = text_width(&label, 8.0, 10.0) + 6.0;
        edge_label_bg(svg, mx, my, lw, 14.0, theme);
        svg.text(
            mx,
            my + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &label,
        );
    }
}

/// Quadratic Bézier from the first to the last point, bent through the routed
/// midpoint as its control point (matching upstream's `M … Q …` rel curves).
/// A straight two-point path collapses to a plain line.
fn quad_path(pts: &[(f64, f64)]) -> String {
    let start = pts[0];
    let end = pts[pts.len() - 1];
    let (mx, my) = polyline_midpoint(pts);
    // Lift the control point so the curve actually passes through the midpoint at t=0.5.
    let cx = 2.0 * mx - (start.0 + end.0) / 2.0;
    let cy = 2.0 * my - (start.1 + end.1) / 2.0;
    format!(
        "M{} {} Q{} {} {} {}",
        fnum(start.0),
        fnum(start.1),
        fnum(cx),
        fnum(cy),
        fnum(end.0),
        fnum(end.1),
    )
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
