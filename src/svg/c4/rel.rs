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
    let clipped = [p_first, p_last];

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
