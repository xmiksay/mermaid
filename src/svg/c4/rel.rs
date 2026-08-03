//! C4 relation drawing: straight point-to-point connectors clipped to each
//! node, with an arrow head and an optional `[techn]` tag.

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

    // Upstream draws a straight line between the two shape borders (#327).
    let p_first = clip_rect((tx, ty), (sx, sy), (aw, ah));
    let p_last = clip_rect((sx, sy), (tx, ty), (bw, bh));

    let markers = if r.bidirectional {
        "marker-start=\"url(#c4-arrow)\" marker-end=\"url(#c4-arrow)\""
    } else {
        "marker-end=\"url(#c4-arrow)\""
    };

    svg.path(
        &format!(
            "M{} {} L{} {}",
            fnum(p_first.0),
            fnum(p_first.1),
            fnum(p_last.0),
            fnum(p_last.1),
        ),
        &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\" {markers}"),
    );

    let label = &r.label;
    let tech = r.technology.as_deref();
    if label.is_empty() && tech.is_none() {
        return;
    }

    let label = truncate(label, 36);
    let tech = tech.map(|t| format!("[{}]", truncate(t, 30)));
    let lw = text_width(&label, 8.0, 10.0) + 6.0;
    let tw = tech.as_deref().map(|t| text_width(t, 8.0, 9.0) + 6.0);

    // Nodes the relation doesn't itself connect to must stay clear of the
    // label — a straight point-to-point line can pass directly through an
    // unrelated node in a cramped row-flow layout, and upstream never lets
    // the label land on its fill (#366).
    let obstacles: Vec<(f64, f64, f64, f64)> = pos
        .iter()
        .filter(|(id, _)| **id != r.from && **id != r.to)
        .map(|(_, &rect)| rect)
        .collect();
    let (mut mx, mut my) = find_clear_label_point(p_first, p_last, lw, tw, &obstacles);
    if let Some(s) = ov {
        mx += s.offset_x.unwrap_or(0.0);
        my += s.offset_y.unwrap_or(0.0);
    }
    // Opaque background behind each label line so the relation label stays
    // legible where it crosses geometry (upstream `edgeLabelBackground`, #260).
    if let Some(tech) = tech {
        let tw = tw.expect("tw is Some whenever tech is Some");
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
        edge_label_bg(svg, mx, my, lw, 14.0, theme);
        svg.text(
            mx,
            my + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &label,
        );
    }
}

/// Slide the label along the relation line — preferring its arc-length
/// midpoint — until its background rect(s) clear every `obstacles` rect.
/// Falls back to the position with the least overlap if the line is too
/// short for a fully clear spot (e.g. two adjacent nodes with barely a gap).
fn find_clear_label_point(
    p0: (f64, f64),
    p1: (f64, f64),
    lw: f64,
    tw: Option<f64>,
    obstacles: &[(f64, f64, f64, f64)],
) -> (f64, f64) {
    if obstacles.is_empty() {
        return polyline_midpoint(&[p0, p1]);
    }
    const STEPS: i32 = 100;
    let mut best = (p0.0 + 0.5 * (p1.0 - p0.0), p0.1 + 0.5 * (p1.1 - p0.1));
    let mut best_overlap = f64::INFINITY;
    for i in 0..=STEPS {
        let k = (i + 1) / 2;
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let t = (0.5 + sign * k as f64 / STEPS as f64).clamp(0.0, 1.0);
        let mx = p0.0 + t * (p1.0 - p0.0);
        let my = p0.1 + t * (p1.1 - p0.1);
        let overlap = label_overlap(mx, my, lw, tw, obstacles);
        if overlap <= 0.0 {
            return (mx, my);
        }
        if overlap < best_overlap {
            best_overlap = overlap;
            best = (mx, my);
        }
    }
    best
}

/// Total overlap area between the label's background rect(s), centered at
/// `(mx, my)`, and every rect in `obstacles`.
fn label_overlap(
    mx: f64,
    my: f64,
    lw: f64,
    tw: Option<f64>,
    obstacles: &[(f64, f64, f64, f64)],
) -> f64 {
    let rects: Vec<(f64, f64, f64, f64)> = match tw {
        Some(tw) => vec![
            (mx - lw / 2.0, my - 10.5, lw, 13.0),
            (mx - tw / 2.0, my + 3.0, tw, 12.0),
        ],
        None => vec![(mx - lw / 2.0, my - 7.0, lw, 14.0)],
    };
    rects
        .iter()
        .flat_map(|rect| {
            obstacles
                .iter()
                .map(move |obs| rect_overlap_area(*rect, *obs))
        })
        .sum()
}

/// Overlap area of two `(x, y, w, h)` rects, 0 if they don't intersect.
fn rect_overlap_area(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let x_overlap = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
    let y_overlap = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
    if x_overlap > 0.0 && y_overlap > 0.0 {
        x_overlap * y_overlap
    } else {
        0.0
    }
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

    #[test]
    fn find_clear_label_point_uses_midpoint_when_unobstructed() {
        let p = find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[]);
        assert_eq!(p, (50.0, 0.0));
    }

    #[test]
    fn find_clear_label_point_dodges_a_node_straddling_the_midpoint() {
        // Mirrors #366: a third node sits directly on the line between the
        // two shapes the relation connects, covering the geometric midpoint.
        let obstacle = (30.0, -10.0, 40.0, 20.0); // x 30..70, y -10..10
        let (mx, my) = find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[obstacle]);
        let label_rect = (mx - 10.0, my - 7.0, 20.0, 14.0);
        assert_eq!(
            rect_overlap_area(label_rect, obstacle),
            0.0,
            "label {label_rect:?} must clear obstacle {obstacle:?}"
        );
        assert_eq!(my, 0.0, "label should stay on the relation line");
    }

    #[test]
    fn find_clear_label_point_falls_back_when_no_gap_exists() {
        // The obstacle covers the whole line, so no clear spot exists — the
        // best-effort fallback must still return a finite point on the line.
        let obstacle = (-100.0, -10.0, 300.0, 20.0);
        let (mx, my) = find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[obstacle]);
        assert!(mx.is_finite() && my.is_finite());
    }

    #[test]
    fn rect_overlap_area_zero_for_disjoint_rects() {
        assert_eq!(
            rect_overlap_area((0.0, 0.0, 10.0, 10.0), (20.0, 20.0, 10.0, 10.0)),
            0.0
        );
    }

    #[test]
    fn rect_overlap_area_positive_for_overlapping_rects() {
        assert_eq!(
            rect_overlap_area((0.0, 0.0, 10.0, 10.0), (5.0, 5.0, 10.0, 10.0)),
            25.0
        );
    }
}
