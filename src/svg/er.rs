//! ER diagram renderer. Entities are drawn as tables (name header + attribute
//! rows), connected by lines with Crow's Foot cardinality markers on each end.

use std::collections::HashMap;

use crate::parse::{Cardinality, Entity, ErDiagram, ErRelation, FlowDirection};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_path, SvgBuilder};
use super::geometry::{clip_rect, polyline_midpoint};
use super::metrics::text_width;
use super::style::resolve_style;
use super::theme::Theme;

const CHAR_W: f64 = 7.5;
const ROW_H: f64 = 30.0; // attribute row height (bordered table cell)
const PAD_X: f64 = 14.0; // header horizontal padding
const CELL_PAD: f64 = 12.0; // attribute-cell horizontal text padding
const HEADER_H: f64 = 28.0;
const MIN_W: f64 = 130.0;
const CANVAS_PAD: f64 = 24.0;
const CARD_GAP: f64 = 14.0; // distance from node boundary to where the cardinality glyph sits

// Crow's-foot marker geometry, measured along the edge from the entity boundary.
const FOOT_TIP: f64 = CARD_GAP + 4.0; // splayed end of the crow's foot (18.0)
const CARD_CIRCLE_R: f64 = 5.0; // radius of the optional-"zero" circle
                                // Center of the zero-or-more circle: one marker length past the foot tip so the
                                // circle reads as clearly separate from the foot (upstream "o{" layout) instead
                                // of merging into it as a "Ø" blob.
const ZERO_MORE_CIRCLE_D: f64 = FOOT_TIP + CARD_CIRCLE_R + 4.0; // 27.0

/// Attribute-table columns, in draw order. Type and name always show; key and
/// comment appear only when some attribute populates them.
#[derive(Clone, Copy)]
enum Col {
    Type,
    Name,
    Key,
    Comment,
}

pub(crate) fn render(d: &ErDiagram, theme: &Theme) -> String {
    if d.entities.is_empty() {
        return SvgBuilder::new(40.0, 40.0).theme(theme).finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d
        .entities
        .iter()
        .map(|e| entity_size(e, theme.font_size))
        .collect();
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
    // Sugiyama only lays out top-down; for LR/RL swap node sizes so the
    // transposed layout reserves the right footprint (as flowchart/class do).
    let node_size_su: HashMap<NodeId, (f64, f64)> = d
        .entities
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let (w, h) = sizes[i];
            let s = match dir {
                FlowDirection::LeftRight | FlowDirection::RightLeft => (h, w),
                _ => (w, h),
            };
            (i as NodeId, s)
        })
        .collect();

    let g = Graph {
        nodes,
        edges,
        node_size: node_size_su,
    };
    let layout = layout_with(&g, &LayoutConfig::default()).unwrap_or_default();
    let (raw_w, raw_h) = (layout.width, layout.height);
    let (canvas_w, canvas_h) = match dir {
        FlowDirection::TopDown | FlowDirection::BottomTop => (raw_w, raw_h),
        FlowDirection::LeftRight | FlowDirection::RightLeft => (raw_h, raw_w),
    };
    let width = canvas_w + CANVAS_PAD * 2.0;
    let height = canvas_h + CANVAS_PAD * 2.0;

    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (tx + CANVAS_PAD, ty + CANVAS_PAD)
    };

    let mut svg = SvgBuilder::new(width, height).theme(theme);

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
        draw_entity(&mut svg, center, sizes[i], e, &d.class_defs, theme);
    }

    svg.finish()
}

fn entity_size(e: &Entity, font_size: f64) -> (f64, f64) {
    let title_w = text_width(&e.label, CHAR_W, font_size) + PAD_X * 2.0;
    if e.attributes.is_empty() {
        return (title_w.max(MIN_W), HEADER_H);
    }
    // The table is exactly as wide as its columns; the header may force it wider.
    let content: f64 = entity_columns(e, font_size).iter().map(|(_, w)| w).sum();
    let w = content.max(title_w).max(MIN_W);
    let h = HEADER_H + e.attributes.len() as f64 * ROW_H;
    (w, h)
}

/// Present attribute columns, each sized to its widest cell plus padding.
fn entity_columns(e: &Entity, font_size: f64) -> Vec<(Col, f64)> {
    let widest = |texts: &mut dyn Iterator<Item = &str>| {
        texts
            .map(|t| text_width(t, CHAR_W, font_size))
            .fold(0.0_f64, f64::max)
    };
    let cell = |w: f64| w + CELL_PAD * 2.0;
    let mut cols = Vec::with_capacity(4);
    cols.push((
        Col::Type,
        cell(widest(&mut e.attributes.iter().map(|a| a.type_.as_str()))),
    ));
    cols.push((
        Col::Name,
        cell(widest(&mut e.attributes.iter().map(|a| a.name.as_str()))),
    ));
    if e.attributes.iter().any(|a| a.key.is_some()) {
        cols.push((
            Col::Key,
            cell(widest(
                &mut e.attributes.iter().filter_map(|a| a.key.as_deref()),
            )),
        ));
    }
    if e.attributes.iter().any(|a| a.comment.is_some()) {
        cols.push((
            Col::Comment,
            cell(widest(
                &mut e.attributes.iter().filter_map(|a| a.comment.as_deref()),
            )),
        ));
    }
    cols
}

/// Column widths after stretching the last column to fill any slack the header
/// forced, so the bordered cells span the full entity width.
fn resolved_columns(e: &Entity, font_size: f64, width: f64) -> Vec<(Col, f64)> {
    let mut cols = entity_columns(e, font_size);
    let content: f64 = cols.iter().map(|(_, w)| w).sum();
    if let Some(last) = cols.last_mut() {
        let extra = width - content;
        if extra > 0.0 {
            last.1 += extra;
        }
    }
    cols
}

fn draw_entity(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    (w, h): (f64, f64),
    e: &Entity,
    class_defs: &HashMap<String, crate::parse::ast::Style>,
    theme: &Theme,
) {
    let rs = resolve_style(class_defs, &e.classes, &e.style);
    let fg = rs.label_fill(&theme.fg).to_string();
    let stroke = rs.stroke_or(&theme.flow_node_stroke).to_string();
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;

    // Header: the entity name over the primary fill. With no attributes the box
    // is the header alone, so unstyled/attribute-less entities stay unchanged.
    let header_h = if e.attributes.is_empty() { h } else { HEADER_H };
    let mut header_attrs = rs.shape_attrs(&theme.flow_node_fill, &stroke, "1.5");
    header_attrs.push_str(" rx=\"2\"");
    svg.rect(x, y, w, header_h, &header_attrs);
    svg.text(
        cx,
        y + header_h / 2.0 + 5.0,
        &format!(
            "text-anchor=\"middle\" fill=\"{fg}\" font-weight=\"bold\"{}",
            rs.text_attrs()
        ),
        &e.label,
    );

    if e.attributes.is_empty() {
        return;
    }

    // Attributes as a true table: one bordered cell per column, row fills
    // alternating between the background and the primary color (upstream's
    // white/lavender striping). Key markers render plain, like any other cell.
    let cols = resolved_columns(e, theme.font_size, w);
    for (i, a) in e.attributes.iter().enumerate() {
        let row_y = y + HEADER_H + i as f64 * ROW_H;
        let row_fill: &str = if i % 2 == 0 {
            &theme.bg
        } else {
            &theme.flow_node_fill
        };
        let text_y = row_y + ROW_H / 2.0 + 4.5;
        let mut cell_x = x;
        for (col, cw) in &cols {
            svg.rect(
                cell_x,
                row_y,
                *cw,
                ROW_H,
                &format!("fill=\"{row_fill}\" stroke=\"{stroke}\" stroke-width=\"1\""),
            );
            let content = match col {
                Col::Type => Some(a.type_.as_str()),
                Col::Name => Some(a.name.as_str()),
                Col::Key => a.key.as_deref(),
                Col::Comment => a.comment.as_deref(),
            };
            if let Some(t) = content {
                svg.text(
                    cell_x + CELL_PAD,
                    text_y,
                    &format!("fill=\"{fg}\" font-size=\"13\""),
                    t,
                );
            }
            cell_x += cw;
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
fn draw_cardinality(
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
    // Three lines fanning out from the anchor toward the far side.
    let spread = 6.0;
    let (ax, ay) = anchor;
    let tip_x = ax + ux * FOOT_TIP;
    let tip_y = ay + uy * FOOT_TIP;
    // Three radial lines
    svg.line(ax, ay, tip_x, tip_y, stroke);
    svg.line(ax, ay, tip_x + px * spread, tip_y + py * spread, stroke);
    svg.line(ax, ay, tip_x - px * spread, tip_y - py * spread, stroke);
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

    // Pull the numeric value of `attr="…"` immediately following `after` in `s`.
    fn attr_after(s: &str, after: &str, attr: &str) -> f64 {
        let tail = &s[s.find(after).expect("marker present") + after.len()..];
        let start = tail.find(attr).expect("attr present") + attr.len() + 2; // attr="
        let end = start + tail[start..].find('"').unwrap();
        tail[start..end].parse().unwrap()
    }

    #[test]
    fn zero_or_more_circle_is_separate_from_foot() {
        // Issue #256: the optional "zero" circle used to sit inside the crow's
        // foot and read as a merged "Ø" blob. Draw the marker along +y from the
        // origin and check the circle clears the foot tip.
        let mut svg = SvgBuilder::new(60.0, 60.0);
        draw_cardinality(
            &mut svg,
            (0.0, 0.0),
            (0.0, 100.0),
            Cardinality::ZeroOrMore,
            &Theme::default(),
        );
        let out = svg.finish();
        // The middle crow's-foot prong runs from the anchor to the foot tip.
        let foot_tip_y = attr_after(&out, "<line", "y2");
        let circle_cy = attr_after(&out, "<circle", "cy");
        let circle_r = attr_after(&out, "<circle", "r");
        // Circle sits beyond the foot tip and does not overlap it.
        assert!(
            circle_cy - circle_r > foot_tip_y,
            "circle (cy={circle_cy}, r={circle_r}) overlaps foot tip {foot_tip_y}",
        );
        // Offset is about one marker length past the foot tip.
        let offset = circle_cy - foot_tip_y;
        assert!(
            (7.0..=12.0).contains(&offset),
            "circle not offset by ~one marker length: {offset}",
        );
    }

    #[test]
    fn entity_with_attributes() {
        let d = build("erDiagram\nCUSTOMER {\nstring name\nstring email PK\n}\nCUSTOMER ||--o{ ORDER : places\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">name<"));
        assert!(svg.contains(">email<"));
        assert!(svg.contains(">PK<"));
    }

    #[test]
    fn attributes_render_as_bordered_striped_table() {
        // Issue #255: attribute rows are a real table — per-cell bordered rects
        // with alternating row fills, not a flat lavender panel.
        let d = build("erDiagram\nCUSTOMER {\nstring name\nstring email PK\nstring phone\n}\n");
        let theme = Theme::default();
        let svg = render(&d, &theme);
        // Cell borders use the theme stroke.
        assert!(svg.contains(&format!(
            "stroke=\"{}\" stroke-width=\"1\"",
            theme.flow_node_stroke
        )));
        // Row striping alternates background (odd) and primary (even) fills.
        assert!(svg.contains(&format!("fill=\"{}\" stroke=", theme.bg)));
        assert!(svg.contains(&format!("fill=\"{}\" stroke=", theme.flow_node_fill)));
    }

    #[test]
    fn key_marker_is_plain_not_red_bold() {
        // Issue #255: PK/FK were rendered red and bold — an invention. They now
        // render plain, like every other attribute cell.
        let d = build("erDiagram\nCUSTOMER {\nstring email PK\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">PK<"));
        assert!(!svg.contains("#c33"));
        assert!(!svg.contains("font-weight=\"bold\" fill=\"#c33\""));
    }

    #[test]
    fn comment_is_rendered() {
        let d = build("erDiagram\nCUSTOMER {\nstring name \"the customer name\"\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">the customer name<"));
    }

    #[test]
    fn alias_label_shown() {
        let d = build("erDiagram\np[Person] {\nstring name\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">Person<"));
    }

    #[test]
    fn classdef_recolors_entity() {
        let d = build(
            "erDiagram\nCUSTOMER ||--o{ ORDER : places\nclassDef hot fill:#ff0000,stroke:#990000\nclass CUSTOMER hot\n",
        );
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#ff0000\""));
        assert!(svg.contains("stroke=\"#990000\""));
    }

    #[test]
    fn unstyled_entity_uses_theme() {
        // Without any classDef the entity box uses the theme fill (regression:
        // the styling path must stay byte-compatible for unstyled diagrams).
        let d = build("erDiagram\nA ||--|| B : x\n");
        let svg = render(&d, &Theme::default());
        let theme = Theme::default();
        assert!(svg.contains(&format!("fill=\"{}\"", theme.flow_node_fill)));
    }
}
