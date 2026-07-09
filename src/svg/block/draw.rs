//! block-beta shape drawing: the recursive `draw` dispatcher, `draw_block`
//! shape rendering, and the polygon path helpers for lean/trapezoid shapes.

use std::collections::HashMap;

use crate::parse::ast::Style;
use crate::parse::{Block, BlockItem, BlockShape};

use crate::svg::builder::{fnum, SvgBuilder};
use crate::svg::style::resolve_style;
use crate::svg::theme::Theme;

use super::edges::arrow_path;
use super::Laid;

pub(super) fn draw(
    l: &Laid,
    svg: &mut SvgBuilder,
    theme: &Theme,
    class_defs: &HashMap<String, Style>,
) {
    match &l.item {
        BlockItem::Block(b) => draw_block(b, l.x, l.y, l.w, l.h, svg, theme, class_defs),
        BlockItem::Group(_) => {
            // Composite container: a solid pale fill hugging its children at
            // natural size, no title text — near-square corners like upstream,
            // not the old large radius (#310).
            svg.rect(
                l.x,
                l.y,
                l.w,
                l.h,
                &format!(
                    "fill=\"{}\" stroke=\"{}\" stroke-width=\"1\" rx=\"1\"",
                    theme.flow_cluster_fill, theme.flow_cluster_stroke
                ),
            );
            if let Some(tf) = &l.child_tf {
                svg.raw(&format!(
                    "<g transform=\"translate({} {}) scale({})\">",
                    fnum(tf.tx),
                    fnum(tf.ty),
                    fnum(tf.s)
                ));
                for c in &l.children {
                    draw(c, svg, theme, class_defs);
                }
                svg.raw("</g>");
            }
        }
        BlockItem::Edge(_) | BlockItem::Space(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_block(
    b: &Block,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
    theme: &Theme,
    class_defs: &HashMap<String, Style>,
) {
    let rs = resolve_style(class_defs, &b.classes, &b.style);
    let attrs = rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5");
    let stroke = rs.stroke_or(&theme.flow_node_stroke);
    let label_fill = rs.label_fill(&theme.fg);
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    match b.shape {
        BlockShape::Rect => svg.rect(x, y, w, h, &attrs),
        BlockShape::Round => svg.rect(x, y, w, h, &format!("{attrs} rx=\"8\"")),
        BlockShape::Stadium => svg.rect(x, y, w, h, &format!("{attrs} rx=\"{}\"", fnum(h / 2.0))),
        BlockShape::Cylinder => {
            svg.rect(x, y, w, h, &attrs);
            svg.line(
                x,
                y + 8.0,
                x + w,
                y + 8.0,
                &format!("stroke=\"{stroke}\" stroke-width=\"1\""),
            );
            svg.line(
                x,
                y + h - 8.0,
                x + w,
                y + h - 8.0,
                &format!("stroke=\"{stroke}\" stroke-width=\"1\""),
            );
        }
        BlockShape::Circle => {
            let r = w.min(h) / 2.0;
            svg.circle(cx, cy, r, &attrs);
        }
        BlockShape::Rhombus => {
            let d = format!(
                "M{cx} {t}L{r} {cy}L{cx} {bt}L{l} {cy}Z",
                cx = fnum(cx),
                cy = fnum(cy),
                t = fnum(y),
                bt = fnum(y + h),
                l = fnum(x),
                r = fnum(x + w),
            );
            svg.path(&d, &attrs);
        }
        BlockShape::Hexagon => {
            let dh = h / 2.0;
            let d = format!(
                "M{l} {cy}L{a} {t}L{b} {t}L{r} {cy}L{b} {bb}L{a} {bb}Z",
                l = fnum(x),
                r = fnum(x + w),
                t = fnum(y),
                bb = fnum(y + h),
                cy = fnum(cy),
                a = fnum(x + dh),
                b = fnum(x + w - dh),
            );
            svg.path(&d, &attrs);
        }
        BlockShape::Subroutine => {
            svg.rect(x, y, w, h, &format!("{attrs} rx=\"2\""));
            svg.line(
                x + 6.0,
                y,
                x + 6.0,
                y + h,
                &format!("stroke=\"{stroke}\" stroke-width=\"1\""),
            );
            svg.line(
                x + w - 6.0,
                y,
                x + w - 6.0,
                y + h,
                &format!("stroke=\"{stroke}\" stroke-width=\"1\""),
            );
        }
        BlockShape::DoubleCircle => {
            let r = w.min(h) / 2.0;
            svg.circle(cx, cy, r, &attrs);
            svg.circle(cx, cy, r - 4.0, &attrs);
        }
        BlockShape::Odd => {
            // `>text]` — flag pointing right.
            let off = 12.0;
            let d = format!(
                "M{} {t}L{r0} {t}L{r} {cy}L{r0} {bb}L{} {bb}Z",
                fnum(x),
                fnum(x),
                t = fnum(y),
                bb = fnum(y + h),
                cy = fnum(cy),
                r0 = fnum(x + w - off),
                r = fnum(x + w),
            );
            svg.path(&d, &attrs);
        }
        BlockShape::LeanRight => svg.path(&lean_path(x, y, w, h, true), &attrs),
        BlockShape::LeanLeft => svg.path(&lean_path(x, y, w, h, false), &attrs),
        BlockShape::Trapezoid => svg.path(&trapezoid_path(x, y, w, h, false), &attrs),
        BlockShape::TrapezoidAlt => svg.path(&trapezoid_path(x, y, w, h, true), &attrs),
        BlockShape::Arrow(arrow) => {
            svg.path(&arrow_path(arrow, x, y, w, h), &attrs);
        }
    }
    svg.text(
        cx,
        cy + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{label_fill}\" font-size=\"13\""),
        &b.label,
    );
}

/// Parallelogram path. `right` leans `/ /` (top pushed right), else `\ \`.
fn lean_path(x: f64, y: f64, w: f64, h: f64, right: bool) -> String {
    let off = 12.0;
    let pts = if right {
        [(x + off, y), (x + w, y), (x + w - off, y + h), (x, y + h)]
    } else {
        [(x, y), (x + w - off, y), (x + w, y + h), (x + off, y + h)]
    };
    poly(&pts)
}

/// Trapezoid path. `alt` is the inverted `\ /` (wide top), else `/ \` (narrow top).
fn trapezoid_path(x: f64, y: f64, w: f64, h: f64, alt: bool) -> String {
    let off = 12.0;
    let pts = if alt {
        [(x, y), (x + w, y), (x + w - off, y + h), (x + off, y + h)]
    } else {
        [(x + off, y), (x + w - off, y), (x + w, y + h), (x, y + h)]
    };
    poly(&pts)
}

fn poly(pts: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (px, py)) in pts.iter().enumerate() {
        d.push_str(if i == 0 { "M" } else { "L" });
        d.push_str(&format!("{} {}", fnum(*px), fnum(*py)));
    }
    d.push('Z');
    d
}
