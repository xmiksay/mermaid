//! block-beta renderer. Grid layout: items flow into cells by column count;
//! a composite `block:id … end` takes one slot as a solid container with its
//! children scaled to hug it (#259).

mod edges;

use std::collections::{BTreeMap, HashMap};

use crate::parse::ast::Style;
use crate::parse::{Block, BlockDiagram, BlockItem, BlockShape};

use super::builder::{fnum, SvgBuilder};
use super::markup::strip_tags;
use super::metrics::text_width;
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

/// Outer canvas margin — upstream's small `diagramPadding`, not the old 30px.
const PAD: f64 = 8.0;
/// Per-glyph width for block labels (shared with the flowchart estimate).
const CHAR_W: f64 = 7.5;
/// Horizontal text padding each side — boxes hug their label (#259).
const PAD_X: f64 = 14.0;
/// Uniform row height; tuned to upstream's compact hug, not the old 60px.
const CELL_H: f64 = 38.0;
/// Floor for the content-derived uniform column width.
const MIN_CELL_W: f64 = 40.0;
const GAP: f64 = 8.0;
/// Inner padding between a composite container and its scaled children.
const GROUP_PAD: f64 = 6.0;

/// Maps a laid-out child frame (local coords) onto its parent frame:
/// `parent = (tx, ty) + s · local`. Composite groups scale their children
/// into a single grid slot, so nested geometry composes these.
#[derive(Clone, Copy)]
struct Transform {
    tx: f64,
    ty: f64,
    s: f64,
}

impl Transform {
    const IDENTITY: Transform = Transform {
        tx: 0.0,
        ty: 0.0,
        s: 1.0,
    };

    /// `self ∘ inner` — apply `inner` (child frame) then `self`.
    fn compose(&self, inner: &Transform) -> Transform {
        Transform {
            tx: self.tx + self.s * inner.tx,
            ty: self.ty + self.s * inner.ty,
            s: self.s * inner.s,
        }
    }
}

#[derive(Clone)]
struct Laid {
    item: BlockItem,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// Children in this node's own (local, pre-scale) coordinate frame.
    children: Vec<Laid>,
    /// For a composite group: maps `children` onto this frame (translate +
    /// scale into the slot). `None` for leaf blocks and edges.
    child_tf: Option<Transform>,
}

pub(crate) fn render(d: &BlockDiagram, theme: &Theme) -> String {
    let (cw, ch) = cell_dims(&d.items, theme.font_size);
    let (laid, total_w, total_h) = layout_items(&d.items, d.columns.unwrap_or(3), PAD, PAD, cw, ch);
    let width = PAD * 2.0 + total_w;
    let height = PAD * 2.0 + total_h;
    let mut svg = SvgBuilder::new(width.max(120.0), height.max(60.0)).theme(theme);

    // Resolve node geometry (recursively) for edges — leaf blocks *and*
    // composite groups, so an edge can target a `block:ID … end` group.
    // `tf` maps each frame onto the absolute canvas so scaled children of a
    // composite group report their on-screen box, not their pre-scale one.
    let mut nodes: BTreeMap<String, Geom> = BTreeMap::new();
    fn collect(laid: &[Laid], out: &mut BTreeMap<String, Geom>, tf: &Transform) {
        for l in laid {
            let cx = tf.tx + tf.s * (l.x + l.w / 2.0);
            let cy = tf.ty + tf.s * (l.y + l.h / 2.0);
            let w = l.w * tf.s;
            let h = l.h * tf.s;
            match &l.item {
                BlockItem::Block(b) => {
                    out.insert(
                        b.id.clone(),
                        Geom {
                            cx,
                            cy,
                            w,
                            h,
                            shape: Some(b.shape),
                        },
                    );
                }
                BlockItem::Group(g) => {
                    if !g.id.is_empty() {
                        out.insert(
                            g.id.clone(),
                            Geom {
                                cx,
                                cy,
                                w,
                                h,
                                shape: None,
                            },
                        );
                    }
                    if let Some(inner) = &l.child_tf {
                        collect(&l.children, out, &tf.compose(inner));
                    }
                }
                _ => {}
            }
        }
    }
    collect(&laid, &mut nodes, &Transform::IDENTITY);

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

/// Uniform grid cell size for a diagram. Columns share one width — the widest
/// label's hug box (text + `PAD_X`), divided down for multi-span blocks so a
/// `d["Wide"]:2` never forces every column wide. Rows share [`CELL_H`].
fn cell_dims(items: &[BlockItem], font_size: f64) -> (f64, f64) {
    fn walk(items: &[BlockItem], font_size: f64, w: &mut f64) {
        for it in items {
            match it {
                BlockItem::Block(b) => {
                    let span = b.span.max(1) as f64;
                    let nat = text_width(&strip_tags(&b.label), CHAR_W, font_size) + PAD_X * 2.0;
                    // Per-column need once the block's own inner gaps are removed.
                    let per_col = (nat - (span - 1.0) * GAP) / span;
                    *w = w.max(per_col);
                }
                BlockItem::Group(g) => walk(&g.items, font_size, w),
                _ => {}
            }
        }
    }
    let mut w = MIN_CELL_W;
    walk(items, font_size, &mut w);
    (w, CELL_H)
}

fn layout_items(
    items: &[BlockItem],
    cols: usize,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
) -> (Vec<Laid>, f64, f64) {
    let mut laid = Vec::new();
    let mut col = 0usize;
    let mut row = 0usize;
    let row_h = ch;
    let cols = cols.max(1);

    for item in items {
        match item {
            BlockItem::Block(b) => {
                let span = b.span.max(1);
                if col + span > cols && col != 0 {
                    col = 0;
                    row += 1;
                }
                let x = x0 + col as f64 * (cw + GAP);
                let y = y0 + row as f64 * (row_h + GAP);
                let w = span as f64 * cw + (span - 1) as f64 * GAP;
                laid.push(Laid {
                    item: item.clone(),
                    x,
                    y,
                    w,
                    h: row_h,
                    children: Vec::new(),
                    child_tf: None,
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
                    child_tf: None,
                });
            }
            BlockItem::Group(g) => {
                // A composite block takes one grid slot (its `span`, default 1),
                // like any leaf — not a whole row. Its children are laid out in
                // their own frame and scaled to hug that slot (#259).
                let span = g.span.max(1);
                if col + span > cols && col != 0 {
                    col = 0;
                    row += 1;
                }
                let x = x0 + col as f64 * (cw + GAP);
                let y = y0 + row as f64 * (row_h + GAP);
                let w = span as f64 * cw + (span - 1) as f64 * GAP;
                let h = row_h;
                let (child_laid, content_w, content_h) =
                    layout_items(&g.items, g.columns.unwrap_or(cols), 0.0, 0.0, cw, ch);
                let avail_w = (w - GROUP_PAD * 2.0).max(1.0);
                let avail_h = (h - GROUP_PAD * 2.0).max(1.0);
                let scale = (avail_w / content_w).min(avail_h / content_h).min(1.0);
                let tf = Transform {
                    tx: x + (w - content_w * scale) / 2.0,
                    ty: y + (h - content_h * scale) / 2.0,
                    s: scale,
                };
                laid.push(Laid {
                    item: item.clone(),
                    x,
                    y,
                    w,
                    h,
                    children: child_laid,
                    child_tf: Some(tf),
                });
                col += span;
                if col >= cols {
                    col = 0;
                    row += 1;
                }
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
        max_x = cols as f64 * cw + (cols - 1) as f64 * GAP;
    }
    if max_y == 0.0 {
        max_y = ch;
    }
    (laid, max_x, max_y)
}

fn draw(l: &Laid, svg: &mut SvgBuilder, theme: &Theme, class_defs: &HashMap<String, Style>) {
    match &l.item {
        BlockItem::Block(b) => draw_block(b, l.x, l.y, l.w, l.h, svg, theme, class_defs),
        BlockItem::Group(_) => {
            // Composite container: a solid pale fill filling one slot, no title
            // text — its children scale down inside it (#259).
            svg.rect(
                l.x,
                l.y,
                l.w,
                l.h,
                &format!(
                    "fill=\"{}\" stroke=\"{}\" stroke-width=\"1\" rx=\"5\"",
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
        // #260: the edge label sits on an opaque background rect so it stays
        // legible where the edge crosses a node.
        let rect = svg.find("fill=\"#fff\" stroke=\"none\"");
        let label = svg.find(">link<");
        assert!(rect.is_some() && rect < label);
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
    fn composite_group_is_solid_untitled_and_scaled() {
        // #259: a composite block draws a solid pale container (theme cluster
        // fill), no dashed frame, no title text, and scales its children in.
        let src = "block-beta\n  columns 3\n  a b c\n  block:group1\n    x y z\n  end\n";
        let svg = render_from(src);
        let t = Theme::default();
        assert!(svg.contains(&format!("fill=\"{}\"", t.flow_cluster_fill)));
        assert!(!svg.contains("stroke-dasharray=\"5 4\""));
        // no bold title label for the group id
        assert!(!svg.contains(">group1<"));
        // children still rendered, inside a scaling group transform
        assert!(svg.contains(">x<") && svg.contains(">z<"));
        assert!(svg.contains("<g transform=\"translate("));
    }

    #[test]
    fn composite_group_occupies_one_slot_not_full_row() {
        // The container hugs a single grid slot, so a sibling that follows it
        // shares the row instead of being pushed below a full-width group.
        let src = "block-beta\n  columns 3\n  block:g\n    x\n  end\n  sib\n";
        let svg = render_from(src);
        // The whole canvas is ~3 columns wide, well under the old full-row size.
        let width = svg
            .split("viewBox=\"0 0 ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap();
        assert!(width < 200.0, "canvas unexpectedly wide: {width}");
    }

    #[test]
    fn edge_to_composite_group() {
        let src = "block-beta\n  block:G\n    x\n  end\n  y\n  G --> y\n";
        let svg = render_from(src);
        // one edge line drawn (marker present) — group id resolves as a node
        assert!(svg.contains("marker-end=\"url(#blockarrow)\""));
    }

    #[test]
    fn cross_and_circle_head_markers() {
        let svg = render_from("block-beta\n  a b c\n  a --x b\n  b --o c\n");
        assert!(svg.contains("marker-end=\"url(#blockcross)\""));
        assert!(svg.contains("marker-end=\"url(#blockcircle)\""));
        assert!(svg.contains("id=\"blockcross\""));
        assert!(svg.contains("id=\"blockcircle\""));
    }

    #[test]
    fn bidirectional_link_marks_both_ends() {
        let svg = render_from("block-beta\n  a b\n  a <--> b\n");
        assert!(svg.contains("marker-start=\"url(#blockarrow)\""));
        assert!(svg.contains("marker-end=\"url(#blockarrow)\""));
    }

    fn render_from(src: &str) -> String {
        match crate::parse::parse(src).unwrap() {
            crate::parse::Diagram::Block(d) => render(&d, &Theme::default()),
            _ => panic!("expected block"),
        }
    }
}
