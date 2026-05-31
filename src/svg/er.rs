//! ER diagram renderer. Entities are drawn as tables (name header + attribute
//! rows), connected by lines with Crow's Foot cardinality markers on each end.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::parse::{Cardinality, Entity, ErDiagram, ErRelation};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 20.0;
const PAD_X: f64 = 14.0;
const HEADER_H: f64 = 28.0;
const MIN_W: f64 = 130.0;
const CANVAS_PAD: f64 = 24.0;
const CARD_GAP: f64 = 14.0; // distance from node boundary to where the cardinality glyph sits

pub(crate) fn render(d: &ErDiagram, theme: &Theme) -> String {
    if d.entities.is_empty() {
        return SvgBuilder::new(40.0, 40.0).finish();
    }

    let sizes: Vec<(f64, f64)> = d.entities.iter().map(entity_size).collect();
    let id_to_u32: HashMap<String, NodeId> = d
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.name.clone(), i as NodeId))
        .collect();
    let nodes: Vec<NodeId> = (0..d.entities.len() as NodeId).collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .relations
        .iter()
        .filter_map(|r| Some((*id_to_u32.get(&r.left)?, *id_to_u32.get(&r.right)?)))
        .collect();
    let node_size_su: HashMap<NodeId, (f64, f64)> = d
        .entities
        .iter()
        .enumerate()
        .map(|(i, _)| (i as NodeId, sizes[i]))
        .collect();

    let g = Graph {
        nodes,
        edges,
        node_size: node_size_su,
    };
    let layout = layout_with(&g, &LayoutConfig::default()).unwrap_or_default();
    let width = layout.width + CANVAS_PAD * 2.0;
    let height = layout.height + CANVAS_PAD * 2.0;

    let transform = |(x, y): (f64, f64)| -> (f64, f64) { (x + CANVAS_PAD, y + CANVAS_PAD) };

    let mut svg = SvgBuilder::new(width, height);

    for rel in &d.relations {
        let (Some(&u), Some(&v)) = (id_to_u32.get(&rel.left), id_to_u32.get(&rel.right)) else {
            continue;
        };
        let Some(raw_pts) = layout.edge_points.get(&(u, v)) else {
            continue;
        };
        if raw_pts.len() < 2 {
            continue;
        }
        let pts: Vec<(f64, f64)> = raw_pts.iter().map(|&p| transform(p)).collect();
        draw_relation(&mut svg, &pts, rel, &sizes, &id_to_u32, theme);
    }

    for (i, e) in d.entities.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        draw_entity(&mut svg, center, sizes[i], e, theme);
    }

    svg.finish()
}

fn entity_size(e: &Entity) -> (f64, f64) {
    let mut max_chars = e.name.chars().count();
    for a in &e.attributes {
        let s = format!("{} {} {}", a.type_, a.name, a.key.clone().unwrap_or_default());
        let len = s.chars().count();
        if len > max_chars {
            max_chars = len;
        }
    }
    let w = (max_chars as f64 * CHAR_W + PAD_X * 2.0).max(MIN_W);
    let h = HEADER_H + e.attributes.len() as f64 * LINE_H + if e.attributes.is_empty() { 0.0 } else { 8.0 };
    (w, h)
}

fn draw_entity(svg: &mut SvgBuilder, (cx, cy): (f64, f64), (w, h): (f64, f64), e: &Entity, theme: &Theme) {
    let fg = theme.fg;
    let flow_node_fill = theme.flow_node_fill;
    let flow_node_stroke = theme.flow_node_stroke;
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    svg.rect(
        x,
        y,
        w,
        h,
        &format!(
            "fill=\"{flow_node_fill}\" stroke=\"{flow_node_stroke}\" stroke-width=\"1.5\" rx=\"2\""
        ),
    );
    svg.text(
        cx,
        y + 19.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-weight=\"bold\""),
        &e.name,
    );
    if !e.attributes.is_empty() {
        svg.line(
            x,
            y + HEADER_H,
            x + w,
            y + HEADER_H,
            &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
        );
        let mut row_y = y + HEADER_H + 6.0;
        for a in &e.attributes {
            row_y += LINE_H - 4.0;
            svg.text(
                x + 8.0,
                row_y,
                &format!("fill=\"{fg}\" font-size=\"13\""),
                &a.type_,
            );
            svg.text(
                x + w / 2.0,
                row_y,
                &format!("fill=\"{fg}\" font-size=\"13\""),
                &a.name,
            );
            if let Some(k) = &a.key {
                svg.text(
                    x + w - 8.0,
                    row_y,
                    &format!("text-anchor=\"end\" fill=\"#c33\" font-size=\"11\" font-weight=\"bold\""),
                    k,
                );
            }
            row_y += 4.0;
        }
    }
}

fn draw_relation(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    rel: &ErRelation,
    sizes: &[(f64, f64)],
    id_to_u32: &HashMap<String, NodeId>,
    theme: &Theme,
) {
    let fg = theme.fg;
    let flow_edge_stroke = theme.flow_edge_stroke;
    let flow_label_bg = theme.flow_label_bg;
    let src = id_to_u32[&rel.left] as usize;
    let dst = id_to_u32[&rel.right] as usize;
    let n = pts.len();
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
    let d = polyline_path(&clipped);
    svg.path(
        &d,
        &format!("fill=\"none\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\"{dash_attr}"),
    );

    // Cardinality glyphs at both endpoints — drawn explicitly as paths so we
    // don't need 8 SVG markers.
    draw_cardinality(svg, clipped[0], clipped[1], rel.left_card, theme);
    draw_cardinality(svg, clipped[n - 1], clipped[n - 2], rel.right_card, theme);

    if !rel.label.is_empty() {
        let mid = midpoint(&clipped);
        let chars = rel.label.chars().count() as f64;
        let w = chars * 7.0 + 8.0;
        let h = 16.0;
        svg.rect(
            mid.0 - w / 2.0,
            mid.1 - h / 2.0,
            w,
            h,
            &format!("fill=\"{flow_label_bg}\" stroke=\"none\""),
        );
        svg.text(
            mid.0,
            mid.1 + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            &rel.label,
        );
    }
}

/// Draw a Crow's Foot glyph at `anchor`, oriented by the direction toward `away`.
fn draw_cardinality(svg: &mut SvgBuilder, anchor: (f64, f64), away: (f64, f64), card: Cardinality, theme: &Theme) {
    let flow_edge_stroke = theme.flow_edge_stroke;
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
                let (ix, iy) = (ax + ux * (CARD_GAP - 4.0 + off), ay + uy * (CARD_GAP - 4.0 + off));
                svg.line(ix + px * 7.0, iy + py * 7.0, ix - px * 7.0, iy - py * 7.0, &stroke);
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
            svg.line(ix + px * 7.0, iy + py * 7.0, ix - px * 7.0, iy - py * 7.0, &stroke);
        }
        Cardinality::OneOrMore => {
            // Tick + crow's foot "|{"
            let (ix, iy) = (ax + ux * (CARD_GAP - 4.0), ay + uy * (CARD_GAP - 4.0));
            svg.line(ix + px * 7.0, iy + py * 7.0, ix - px * 7.0, iy - py * 7.0, &stroke);
            draw_crowfoot(svg, anchor, ux, uy, px, py, &stroke);
        }
        Cardinality::ZeroOrMore => {
            // Circle + crow's foot "o{"
            svg.circle(
                ax + ux * (CARD_GAP - 4.0),
                ay + uy * (CARD_GAP - 4.0),
                5.0,
                &format!("fill=\"#fff\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\""),
            );
            draw_crowfoot(svg, anchor, ux, uy, px, py, &stroke);
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
    // Three lines fanning out from the anchor toward the far side.
    let len = 10.0;
    let spread = 6.0;
    let (ax, ay) = anchor;
    let tip_x = ax + ux * (CARD_GAP + 4.0);
    let tip_y = ay + uy * (CARD_GAP + 4.0);
    // Three radial lines
    svg.line(ax, ay, tip_x, tip_y, stroke);
    svg.line(ax, ay, tip_x + px * spread, tip_y + py * spread, stroke);
    svg.line(ax, ay, tip_x - px * spread, tip_y - py * spread, stroke);
    let _ = len;
}

fn polyline_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}

fn clip_rect(from: (f64, f64), c: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - c.0;
    let dy = from.1 - c.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let tx = if dx.abs() > 1e-9 { hw / dx.abs() } else { f64::INFINITY };
    let ty = if dy.abs() > 1e-9 { hh / dy.abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    (c.0 + dx * t, c.1 + dy * t)
}

fn midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return pts[0];
    }
    let mut segs = Vec::with_capacity(pts.len() - 1);
    let mut total = 0.0;
    for w in pts.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let l = (dx * dx + dy * dy).sqrt();
        segs.push(l);
        total += l;
    }
    let half = total / 2.0;
    let mut walked = 0.0;
    for (i, w) in pts.windows(2).enumerate() {
        if walked + segs[i] >= half {
            let t = (half - walked) / segs[i].max(1e-9);
            return (w[0].0 + t * (w[1].0 - w[0].0), w[0].1 + t * (w[1].1 - w[0].1));
        }
        walked += segs[i];
    }
    pts[pts.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> ErDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Er(e) => e,
            _ => panic!("not er"),
        }
    }

    #[test]
    fn renders_basic() {
        let d = build("erDiagram\nCUSTOMER ||--o{ ORDER : places\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">CUSTOMER<"));
        assert!(svg.contains(">ORDER<"));
        assert!(svg.contains(">places<"));
    }

    #[test]
    fn entity_with_attributes() {
        let d = build("erDiagram\nCUSTOMER {\nstring name\nstring email PK\n}\nCUSTOMER ||--o{ ORDER : places\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">name<"));
        assert!(svg.contains(">email<"));
        assert!(svg.contains(">PK<"));
    }
}
