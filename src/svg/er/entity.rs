//! Entity boxes: table sizing (header + attribute columns) and drawing.

use std::collections::HashMap;

use crate::parse::Entity;

use crate::svg::builder::SvgBuilder;
use crate::svg::metrics::text_width;
use crate::svg::style::resolve_style;
use crate::svg::theme::Theme;

const CHAR_W: f64 = 7.5;
const ROW_H: f64 = 30.0; // attribute row height (bordered table cell)
const PAD_X: f64 = 14.0; // header horizontal padding
const CELL_PAD: f64 = 12.0; // attribute-cell horizontal text padding
const HEADER_H: f64 = 28.0;
const MIN_W: f64 = 130.0;

/// Attribute-table columns, in draw order. Type and name always show; key and
/// comment appear only when some attribute populates them.
#[derive(Clone, Copy)]
enum Col {
    Type,
    Name,
    Key,
    Comment,
}

pub(super) fn entity_size(e: &Entity, font_size: f64) -> (f64, f64) {
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

pub(super) fn draw_entity(
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
