//! block-beta renderer. Grid layout: items flow into cells by column count,
//! groups draw a labeled box around inner items.

mod edges;

use std::collections::{BTreeMap, HashMap};

use crate::parse::ast::Style;
use crate::parse::{Block, BlockDiagram, BlockItem, BlockShape};

use super::builder::{fnum, SvgBuilder};
use super::style::resolve_style;
use super::theme::Theme;

use edges::{arrow_path, draw_edge};

/// Center + bounding box + shape of a laid-out node, keyed by id for edge
/// routing. `shape` is `None` for a composite group (clipped as a rectangle).
pub(super) struct Geom {
    pub(super) cx: f64,
    pub(super) cy: f64,
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) shape: Option<BlockShape>,
}

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
    let mut svg = SvgBuilder::new(width.max(200.0), height.max(100.0))
        .font(theme.font_family, theme.font_size);

    // Resolve node geometry (recursively) for edges — leaf blocks *and*
    // composite groups, so an edge can target a `block:ID … end` group.
    let mut nodes: BTreeMap<String, Geom> = BTreeMap::new();
    fn collect(laid: &[Laid], out: &mut BTreeMap<String, Geom>) {
        for l in laid {
            match &l.item {
                BlockItem::Block(b) => {
                    out.insert(
                        b.id.clone(),
                        Geom {
                            cx: l.x + l.w / 2.0,
                            cy: l.y + l.h / 2.0,
                            w: l.w,
                            h: l.h,
                            shape: Some(b.shape),
                        },
                    );
                }
                BlockItem::Group(g) => {
                    if !g.id.is_empty() {
                        out.insert(
                            g.id.clone(),
                            Geom {
                                cx: l.x + l.w / 2.0,
                                cy: l.y + l.h / 2.0,
                                w: l.w,
                                h: l.h,
                                shape: None,
                            },
                        );
                    }
                    collect(&l.children, out);
                }
                _ => {}
            }
        }
    }
    collect(&laid, &mut nodes);

    // Draw items.
    for l in &laid {
        draw(l, &mut svg, theme, &d.class_defs);
    }
    // Draw edges last so they sit on top.
    for l in &laid {
        if let BlockItem::Edge(e) = &l.item {
            draw_edge(e, &nodes, &mut svg, theme);
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
                // Honor `block:id:span` — the group is at least `span` cells wide.
                let span_w = g.span.max(1) as f64 * CELL_W + (g.span.max(1) - 1) as f64 * GAP;
                let w = (cw + GROUP_PAD * 2.0).max(span_w);
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

fn draw(l: &Laid, svg: &mut SvgBuilder, theme: &Theme, class_defs: &HashMap<String, Style>) {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    match &l.item {
        BlockItem::Block(b) => draw_block(b, l.x, l.y, l.w, l.h, svg, theme, class_defs),
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
                draw(c, svg, theme, class_defs);
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
    let attrs = rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5");
    let stroke = rs.stroke_or(theme.flow_node_stroke);
    let label_fill = rs.label_fill(theme.fg);
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
                    ..Block::default()
                }),
                BlockItem::Block(Block {
                    id: "b".into(),
                    label: "B".into(),
                    shape: BlockShape::Circle,
                    span: 1,
                    ..Block::default()
                }),
            ],
            ..BlockDiagram::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains(">B<"));
    }

    #[test]
    fn classdef_style_and_edge_label() {
        let src = "block-beta\n  columns 2\n  a b\n  classDef hot fill:#f00,stroke:#900\n  class a hot\n  a -- \"link\" --> b\n";
        let svg = render_from(src);
        assert!(svg.contains("#f00"));
        assert!(svg.contains(">link<"));
        // no ghost blocks for the classDef/class keywords
        assert!(!svg.contains(">classDef<"));
        assert!(!svg.contains(">hot<"));
    }

    #[test]
    fn block_arrow_renders_path() {
        let svg = render_from("block-beta\n  a<[\"go\"]>(right)\n");
        assert!(svg.contains("<path"));
        assert!(svg.contains(">go<"));
    }

    #[test]
    fn edge_to_composite_group() {
        let src = "block-beta\n  block:G\n    x\n  end\n  y\n  G --> y\n";
        let svg = render_from(src);
        // one edge line drawn (marker present) — group id resolves as a node
        assert!(svg.contains("marker-end=\"url(#blockarrow)\""));
    }

    fn render_from(src: &str) -> String {
        match crate::parse::parse(src).unwrap() {
            crate::parse::Diagram::Block(d) => render(&d, &Theme::default()),
            _ => panic!("expected block"),
        }
    }
}
