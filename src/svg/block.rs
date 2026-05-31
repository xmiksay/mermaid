//! block-beta renderer. Grid layout: items flow into cells by column count,
//! groups draw a labeled box around inner items.

use std::collections::BTreeMap;

use crate::parse::{Block, BlockDiagram, BlockEdge, BlockItem, BlockShape};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const CELL_W: f64 = 100.0;
const CELL_H: f64 = 60.0;
const GAP: f64 = 8.0;
const GROUP_PAD: f64 = 14.0;

#[derive(Clone)]
struct Laid {
    item: BlockItem,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    children: Vec<Laid>,
}

pub(crate) fn render(d: &BlockDiagram, theme: &Theme) -> String {
    let (laid, total_w, total_h) = layout_items(&d.items, d.columns.unwrap_or(3), PAD, PAD);
    let width = PAD * 2.0 + total_w;
    let height = PAD * 2.0 + total_h + 20.0;
    let mut svg = SvgBuilder::new(width.max(200.0), height.max(100.0));

    // Resolve block centers (recursively) for edges.
    let mut centers: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    fn collect(laid: &[Laid], out: &mut BTreeMap<String, (f64, f64)>) {
        for l in laid {
            if let BlockItem::Block(b) = &l.item {
                out.insert(b.id.clone(), (l.x + l.w / 2.0, l.y + l.h / 2.0));
            }
            if let BlockItem::Group(_) = &l.item {
                collect(&l.children, out);
            }
        }
    }
    collect(&laid, &mut centers);

    // Draw items.
    for l in &laid {
        draw(l, &mut svg, theme);
    }
    // Draw edges last so they sit on top.
    for l in &laid {
        if let BlockItem::Edge(e) = &l.item {
            draw_edge(e, &centers, &mut svg, theme);
        }
    }

    svg.finish()
}

fn layout_items(items: &[BlockItem], cols: usize, x0: f64, y0: f64) -> (Vec<Laid>, f64, f64) {
    let mut laid = Vec::new();
    let mut col = 0usize;
    let mut row = 0usize;
    let row_h = CELL_H;
    let cols = cols.max(1);

    for item in items {
        match item {
            BlockItem::Block(b) => {
                let span = b.span.max(1);
                if col + span > cols && col != 0 {
                    col = 0;
                    row += 1;
                }
                let x = x0 + col as f64 * (CELL_W + GAP);
                let y = y0 + row as f64 * (row_h + GAP);
                let w = span as f64 * CELL_W + (span - 1) as f64 * GAP;
                laid.push(Laid {
                    item: item.clone(),
                    x,
                    y,
                    w,
                    h: row_h,
                    children: Vec::new(),
                });
                col += span;
                if col >= cols {
                    col = 0;
                    row += 1;
                }
            }
            BlockItem::Space(n) => {
                col += n;
                if col >= cols {
                    col = 0;
                    row += 1;
                }
            }
            BlockItem::Edge(_) => {
                laid.push(Laid {
                    item: item.clone(),
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                    children: Vec::new(),
                });
            }
            BlockItem::Group(g) => {
                if col != 0 {
                    col = 0;
                    row += 1;
                }
                let inner_x = x0 + GROUP_PAD;
                let inner_y = y0 + row as f64 * (row_h + GAP) + GROUP_PAD + 8.0;
                let (child_laid, cw, ch) =
                    layout_items(&g.items, g.columns.unwrap_or(cols), inner_x, inner_y);
                let w = cw + GROUP_PAD * 2.0;
                let h = ch + GROUP_PAD * 2.0 + 18.0;
                laid.push(Laid {
                    item: item.clone(),
                    x: x0,
                    y: y0 + row as f64 * (row_h + GAP),
                    w,
                    h,
                    children: child_laid,
                });
                // Group takes whole rows; advance row pointer.
                let rows_used = ((h + GAP) / (row_h + GAP)).ceil() as usize;
                row += rows_used;
                let _ = col;
            }
        }
    }

    let mut max_x: f64 = 0.0;
    let mut max_y: f64 = 0.0;
    for l in &laid {
        if !matches!(l.item, BlockItem::Edge(_)) {
            max_x = max_x.max(l.x + l.w - x0);
            max_y = max_y.max(l.y + l.h - y0);
        }
    }
    if max_x == 0.0 {
        max_x = cols as f64 * CELL_W + (cols - 1) as f64 * GAP;
    }
    if max_y == 0.0 {
        max_y = CELL_H;
    }
    (laid, max_x, max_y)
}

fn draw(l: &Laid, svg: &mut SvgBuilder, theme: &Theme) {
    let fg = theme.fg;
    let fill = theme.flow_node_fill;
    let stroke = theme.flow_node_stroke;
    let fg_muted = theme.fg_muted;
    match &l.item {
        BlockItem::Block(b) => draw_block(b, l.x, l.y, l.w, l.h, svg, fg, fill, stroke),
        BlockItem::Group(g) => {
            svg.rect(l.x, l.y, l.w, l.h,
                &format!("fill=\"none\" stroke=\"{fg_muted}\" stroke-width=\"1.5\" stroke-dasharray=\"5 4\" rx=\"4\""));
            if !g.id.is_empty() {
                svg.text(
                    l.x + 8.0,
                    l.y + 14.0,
                    &format!("fill=\"{fg}\" font-size=\"11\" font-weight=\"bold\""),
                    &g.id,
                );
            }
            for c in &l.children {
                draw(c, svg, theme);
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
    fg: &str,
    fill: &str,
    stroke: &str,
) {
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    match b.shape {
        BlockShape::Rect => svg.rect(
            x,
            y,
            w,
            h,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
        ),
        BlockShape::Round => svg.rect(
            x,
            y,
            w,
            h,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"8\""),
        ),
        BlockShape::Stadium => svg.rect(
            x,
            y,
            w,
            h,
            &format!(
                "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"{}\"",
                fnum(h / 2.0)
            ),
        ),
        BlockShape::Cylinder => {
            svg.rect(
                x,
                y,
                w,
                h,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
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
            svg.circle(
                cx,
                cy,
                r,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
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
            svg.path(
                &d,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
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
            svg.path(
                &d,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
        }
    }
    svg.text(
        cx,
        cy + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
        &b.label,
    );
}

fn draw_edge(
    e: &BlockEdge,
    centers: &BTreeMap<String, (f64, f64)>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let stroke = theme.flow_edge_stroke;
    let (Some(a), Some(b)) = (centers.get(&e.from), centers.get(&e.to)) else {
        return;
    };
    let marker = if e.arrow {
        " marker-end=\"url(#blockarrow)\""
    } else {
        ""
    };
    if e.arrow {
        svg.defs_raw(
            "<marker id=\"blockarrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 Z\" fill=\"#333\"/></marker>"
        );
    }
    svg.line(
        a.0,
        a.1,
        b.0,
        b.1,
        &format!("stroke=\"{stroke}\" stroke-width=\"1.5\"{marker}"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_svg() {
        let d = BlockDiagram {
            columns: Some(2),
            items: vec![
                BlockItem::Block(Block {
                    id: "a".into(),
                    label: "A".into(),
                    shape: BlockShape::Rect,
                    span: 1,
                }),
                BlockItem::Block(Block {
                    id: "b".into(),
                    label: "B".into(),
                    shape: BlockShape::Circle,
                    span: 1,
                }),
            ],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains(">B<"));
    }
}
