//! Relationship lines with Crow's Foot cardinality markers on each end.

use std::collections::HashMap;

use crate::parse::{Cardinality, ErRelation};
use crate::sugiyama::NodeId;

use crate::svg::builder::{curve_basis_path, SvgBuilder};
use crate::svg::geometry::{clip_rect, polyline_midpoint};
use crate::svg::theme::Theme;

pub(super) const CARD_GAP: f64 = 14.0; // distance from node boundary to where the cardinality glyph sits

// Crow's-foot marker geometry, measured along the edge from the entity boundary.
// The prongs are wide at the entity border and converge to a single point one
// marker length out along the edge (upstream's crow's foot spreads outward at
// the entity, not at the connector).
const FOOT_DEPTH: f64 = CARD_GAP + 4.0; // convergence point of the crow's foot (18.0)
const CARD_CIRCLE_R: f64 = 5.0; // radius of the optional-"zero" circle
                                // Center of the zero-or-more circle: tangent to the foot's convergence point
                                // so the circle reads as a complete, separate glyph (upstream "o{" layout)
                                // instead of merging into the foot as a "Ø" blob.
const ZERO_MORE_CIRCLE_D: f64 = FOOT_DEPTH + CARD_CIRCLE_R; // 23.0

pub(super) fn draw_relation(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    rel: &ErRelation,
    sizes: &[(f64, f64)],
    id_to_u32: &HashMap<String, NodeId>,
    theme: &Theme,
) {
    let flow_edge_stroke = &theme.flow_edge_stroke;
    let src = id_to_u32[&rel.left] as usize;
    let dst = id_to_u32[&rel.right] as usize;
    let n = pts.len();
    // The router always yields >= 2 points (both endpoints); guard the
    // invariant so a future regression clips nothing instead of panicking on
    // `pts[1]` / `pts[n - 2]` (the latter underflows `usize`).
    if n < 2 {
        return;
    }
    let first = clip_rect(pts[1], pts[0], sizes[src]);
    let last = clip_rect(pts[n - 2], pts[n - 1], sizes[dst]);

    let mut clipped = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let dash = if rel.identifying { "" } else { "4 3" };
    let dash_attr = if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{dash}\"")
    };
    let d = curve_basis_path(&clipped);
    svg.path(
        &d,
        &format!("fill=\"none\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"{dash_attr}"),
    );

    // Cardinality glyphs at both endpoints — drawn explicitly as paths so we
    // don't need 8 SVG markers.
    draw_cardinality(svg, clipped[0], clipped[1], rel.left_card, theme);
    draw_cardinality(svg, clipped[n - 1], clipped[n - 2], rel.right_card, theme);

    if !rel.label.is_empty() {
        let mid = polyline_midpoint(&clipped);
        crate::svg::label::draw_edge_label(svg, mid, &rel.label, theme);
    }
}

/// Draw a Crow's Foot glyph at `anchor`, oriented by the direction toward `away`.
pub(super) fn draw_cardinality(
    svg: &mut SvgBuilder,
    anchor: (f64, f64),
    away: (f64, f64),
    card: Cardinality,
    theme: &Theme,
) {
    let flow_edge_stroke = &theme.flow_edge_stroke;
    let (ax, ay) = anchor;
    let dx = away.0 - ax;
    let dy = away.1 - ay;
    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
    let ux = dx / len;
    let uy = dy / len;
    // Perpendicular
    let px = -uy;
    let py = ux;

    // Place the glyph CARD_GAP units along the line, centered.
    let cx_glyph = ax + ux * CARD_GAP;
    let cy_glyph = ay + uy * CARD_GAP;

    let stroke = format!("stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\" fill=\"none\"");

    match card {
        Cardinality::ExactlyOne => {
            // Two perpendicular ticks "||"
            for off in [0.0, 6.0] {
                let (ix, iy) = (
                    ax + ux * (CARD_GAP - 4.0 + off),
                    ay + uy * (CARD_GAP - 4.0 + off),
                );
                svg.line(
                    ix + px * 7.0,
                    iy + py * 7.0,
                    ix - px * 7.0,
                    iy - py * 7.0,
                    &stroke,
                );
            }
        }
        Cardinality::ZeroOrOne => {
            // Circle + one tick "o|"
            svg.circle(
                ax + ux * (CARD_GAP - 4.0),
                ay + uy * (CARD_GAP - 4.0),
                5.0,
                &format!("fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\""),
            );
            let (ix, iy) = (ax + ux * (CARD_GAP + 4.0), ay + uy * (CARD_GAP + 4.0));
            svg.line(
                ix + px * 7.0,
                iy + py * 7.0,
                ix - px * 7.0,
                iy - py * 7.0,
                &stroke,
            );
        }
        Cardinality::OneOrMore => {
            // Tick + crow's foot "|{"
            let (ix, iy) = (ax + ux * (CARD_GAP - 4.0), ay + uy * (CARD_GAP - 4.0));
            svg.line(
                ix + px * 7.0,
                iy + py * 7.0,
                ix - px * 7.0,
                iy - py * 7.0,
                &stroke,
            );
            draw_crowfoot(svg, anchor, ux, uy, px, py, &stroke);
        }
        Cardinality::ZeroOrMore => {
            // Crow's foot near the entity, then the "zero" circle set one marker
            // length further along the edge so the two glyphs stay separate ("o{").
            draw_crowfoot(svg, anchor, ux, uy, px, py, &stroke);
            svg.circle(
                ax + ux * ZERO_MORE_CIRCLE_D,
                ay + uy * ZERO_MORE_CIRCLE_D,
                CARD_CIRCLE_R,
                &format!("fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\""),
            );
        }
    }
    let _ = (cx_glyph, cy_glyph);
}

fn draw_crowfoot(
    svg: &mut SvgBuilder,
    anchor: (f64, f64),
    ux: f64,
    uy: f64,
    px: f64,
    py: f64,
    stroke: &str,
) {
    // Three prongs converging at a single point one marker length out, splaying
    // across the entity border — wide at the entity, narrow at the connector.
    let spread = 6.0;
    let (ax, ay) = anchor;
    let point_x = ax + ux * FOOT_DEPTH;
    let point_y = ay + uy * FOOT_DEPTH;
    svg.line(point_x, point_y, ax, ay, stroke);
    svg.line(point_x, point_y, ax + px * spread, ay + py * spread, stroke);
    svg.line(point_x, point_y, ax - px * spread, ay - py * spread, stroke);
}
