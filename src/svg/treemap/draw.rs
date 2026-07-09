//! Per-cell drawing: fills, ink-color selection, leaf labels and section
//! headers, and the per-cell clip paths that keep text inside its rectangle.

use crate::parse::ast::Style;
use crate::parse::TreemapNode;

use crate::svg::builder::{fnum, SvgBuilder};
use crate::svg::style::resolve_style;
use crate::svg::theme::Theme;

use super::format::format_value;
use super::{node_value, Ctx, Rect};

pub(super) fn draw_node(
    n: &TreemapNode,
    r: Rect,
    color: &str,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
) {
    let leaf = n.children.is_empty();
    // A `:::class` reference overrides the branch fill/stroke.
    let classes: Vec<String> = n.class_name.iter().cloned().collect();
    let rs = resolve_style(ctx.class_defs, &classes, &Style::new());
    // Sections and their leaves share one flat hue (upstream draws no
    // per-sibling shading); white strokes keep adjacent cells legible.
    let fill = rs.fill.clone().unwrap_or_else(|| color.to_string());
    let stroke = rs.stroke.as_deref().unwrap_or("#ffffff");
    svg.rect(
        r.x,
        r.y,
        r.w,
        r.h,
        &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\""),
    );
    let ink = text_color(&fill, ctx.theme);
    if leaf {
        draw_leaf_label(n, r, &ink, svg, ctx, next_id);
    } else {
        draw_section_header(n, r, &ink, svg, ctx, next_id);
    }
}

/// Pick white or the theme foreground for text drawn on `fill`, by luminance —
/// upstream uses white on its darker section fills, dark ink on light ones.
pub(super) fn text_color(fill: &str, theme: &Theme) -> String {
    match parse_hex(fill) {
        Some((r, g, b)) => {
            let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            if lum < 140.0 {
                "#ffffff".to_string()
            } else {
                theme.fg.to_string()
            }
        }
        None => theme.fg.to_string(),
    }
}

/// Centered name over its value, clipped to the cell.
fn draw_leaf_label(
    n: &TreemapNode,
    r: Rect,
    ink: &str,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
) {
    if r.w <= 24.0 || r.h <= 16.0 {
        return;
    }
    let value_text = if ctx.show_values && r.h > 30.0 {
        n.value.map(|v| format_value(v, ctx.value_format))
    } else {
        None
    };
    let cx = r.x + r.w / 2.0;
    let cy = r.y + r.h / 2.0;
    let clip = clip_open(r, svg, next_id);
    let name_y = if value_text.is_some() {
        cy - 2.0
    } else {
        cy + 5.0
    };
    svg.text(
        cx,
        name_y,
        &format!("text-anchor=\"middle\" fill=\"{ink}\" font-size=\"16\""),
        &n.label,
    );
    if let Some(v) = value_text {
        svg.text(
            cx,
            cy + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{ink}\" font-size=\"12\""),
            &v,
        );
    }
    clip_close(clip, svg);
}

/// Section band: name left-aligned, running total right-aligned in italics.
fn draw_section_header(
    n: &TreemapNode,
    r: Rect,
    ink: &str,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
) {
    if r.w <= 30.0 || r.h <= 16.0 {
        return;
    }
    let clip = clip_open(r, svg, next_id);
    let y = r.y + 15.0;
    svg.text(
        r.x + 6.0,
        y,
        &format!("text-anchor=\"start\" fill=\"{ink}\" font-size=\"13\" font-weight=\"bold\""),
        &n.label,
    );
    if ctx.show_values {
        svg.text(
            r.x + r.w - 6.0,
            y,
            &format!("text-anchor=\"end\" fill=\"{ink}\" font-size=\"12\" font-style=\"italic\""),
            &format_value(node_value(n), ctx.value_format),
        );
    }
    clip_close(clip, svg);
}

/// Register a per-cell clip path and open a `<g>` bound to it. Returns the id.
fn clip_open(r: Rect, svg: &mut SvgBuilder, next_id: &mut usize) -> usize {
    let id = *next_id;
    *next_id += 1;
    svg.defs_raw(&format!(
        "<clipPath id=\"tm-clip-{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
        fnum(r.x),
        fnum(r.y),
        fnum(r.w),
        fnum(r.h)
    ));
    svg.raw(&format!("<g clip-path=\"url(#tm-clip-{id})\">"));
    id
}

fn clip_close(_id: usize, svg: &mut SvgBuilder) {
    svg.raw("</g>");
}

/// Parse `#rgb`/`#rrggbb` into RGB bytes; `None` for any other syntax.
fn parse_hex(c: &str) -> Option<(u8, u8, u8)> {
    let h = c.strip_prefix('#')?;
    let (r, g, b) = match h.len() {
        6 => (&h[0..2], &h[2..4], &h[4..6]),
        3 => {
            return parse_hex(&format!(
                "#{a}{a}{b}{b}{c}{c}",
                a = &h[0..1],
                b = &h[1..2],
                c = &h[2..3]
            ))
        }
        _ => return None,
    };
    Some((
        u8::from_str_radix(r, 16).ok()?,
        u8::from_str_radix(g, 16).ok()?,
        u8::from_str_radix(b, 16).ok()?,
    ))
}
