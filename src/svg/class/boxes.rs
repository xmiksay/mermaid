//! Class box sizing and drawing: the three-compartment box (name, attributes,
//! methods) with its optional stereotype line and per-member styling.

use std::collections::HashMap;

use crate::parse::{MemberKind, Style, UmlClass};

use super::super::builder::SvgBuilder;
use super::super::interact::{close_click, open_click};
use super::super::metrics::font_scale;
use super::super::style::resolve_style;
use super::super::theme::Theme;
use super::members::{convert_generics, member_display};
use super::{CHAR_W, COMPARTMENT_PAD, EMPTY_COMPARTMENT_H, HEADER_PAD, LINE_H, MIN_W, PAD_X};

/// The header text drawn in a class box: the explicit `["label"]` if any, else
/// the class name with generic `~T~` converted to `<T>`.
fn class_display(c: &UmlClass) -> String {
    c.label.clone().unwrap_or_else(|| convert_generics(&c.name))
}

pub(super) fn class_size(c: &UmlClass, font_size: f64) -> (f64, f64) {
    let mut max_chars = class_display(c).chars().count();
    if let Some(s) = &c.stereotype {
        max_chars = max_chars.max(s.chars().count() + 4);
    }
    let attr_lines = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Attribute)
        .count();
    let meth_lines = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Method)
        .count();
    for m in &c.members {
        let len = member_display(m).text.chars().count();
        if len > max_chars {
            max_chars = len;
        }
    }
    let w = (max_chars as f64 * CHAR_W * font_scale(font_size) + PAD_X * 2.0).max(MIN_W);
    let header_h = if c.stereotype.is_some() {
        HEADER_PAD + LINE_H
    } else {
        HEADER_PAD
    };
    let attr_h = if attr_lines == 0 {
        EMPTY_COMPARTMENT_H
    } else {
        attr_lines as f64 * LINE_H + COMPARTMENT_PAD * 2.0
    };
    let meth_h = if meth_lines == 0 {
        EMPTY_COMPARTMENT_H
    } else {
        meth_lines as f64 * LINE_H + COMPARTMENT_PAD * 2.0
    };
    let h = header_h + attr_h + meth_h + 4.0;
    (w, h)
}

fn draw_member(svg: &mut SvgBuilder, x: f64, y: f64, m: &crate::parse::ClassMember, fg: &str) {
    let md = member_display(m);
    let mut attrs = format!("fill=\"{fg}\" font-size=\"13\"");
    if md.is_abstract {
        attrs.push_str(" font-style=\"italic\"");
    }
    if md.is_static {
        attrs.push_str(" text-decoration=\"underline\"");
    }
    svg.text(x, y, &attrs, &md.text);
}

pub(super) fn draw_class(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    (w, h): (f64, f64),
    c: &UmlClass,
    class_defs: &HashMap<String, Style>,
    theme: &Theme,
) {
    if let Some(action) = &c.click {
        open_click(svg, action);
    }
    let rs = resolve_style(class_defs, &c.classes, &c.style);
    let fg = rs.label_fill(&theme.fg);
    let flow_node_stroke = rs.stroke_or(&theme.flow_node_stroke);
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let base = rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5");
    svg.rect(x, y, w, h, &format!("{base} rx=\"2\""));

    let mut cursor = y;
    // Header (with optional stereotype line above the name).
    if let Some(s) = &c.stereotype {
        cursor += 16.0;
        svg.text(
            cx,
            cursor,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            &format!("«{s}»"),
        );
    } else {
        cursor += 6.0;
    }
    cursor += LINE_H;
    svg.text(
        cx,
        cursor,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-weight=\"bold\""),
        &class_display(c),
    );
    cursor += 4.0;

    let attrs: Vec<_> = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Attribute)
        .collect();
    let meths: Vec<_> = c
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Method)
        .collect();

    // Attributes compartment (always drawn: upstream keeps the three-compartment
    // box even when a section is empty).
    cursor += 4.0;
    svg.line(
        x,
        cursor,
        x + w,
        cursor,
        &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
    );
    if attrs.is_empty() {
        cursor += EMPTY_COMPARTMENT_H - 4.0;
    } else {
        cursor += COMPARTMENT_PAD;
        for m in attrs {
            cursor += LINE_H - 4.0;
            draw_member(svg, x + 8.0, cursor, m, fg);
            cursor += 4.0;
        }
        cursor += COMPARTMENT_PAD - 4.0;
    }

    // Methods compartment (always drawn).
    svg.line(
        x,
        cursor,
        x + w,
        cursor,
        &format!("stroke=\"{flow_node_stroke}\" stroke-width=\"1\""),
    );
    if !meths.is_empty() {
        cursor += COMPARTMENT_PAD;
        for m in meths {
            cursor += LINE_H - 4.0;
            draw_member(svg, x + 8.0, cursor, m, fg);
            cursor += 4.0;
        }
    }

    if let Some(action) = &c.click {
        close_click(svg, action);
    }
}
