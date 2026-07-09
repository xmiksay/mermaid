//! block-beta grid layout: uniform cell sizing (`cell_dims`) and the recursive
//! column-flow placement (`layout_items`) that hugs composite groups.

use crate::parse::BlockItem;

use crate::svg::markup::strip_tags;
use crate::svg::metrics::text_width;

use super::{Laid, Transform, CELL_H, CHAR_W, GAP, GROUP_PAD, MIN_CELL_W, PAD_X};

/// Uniform grid cell size for a diagram. Columns share one width — the widest
/// label's hug box (text + `PAD_X`), divided down for multi-span blocks so a
/// `d["Wide"]:2` never forces every column wide. Rows share [`CELL_H`].
pub(super) fn cell_dims(items: &[BlockItem], font_size: f64) -> (f64, f64) {
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

pub(super) fn layout_items(
    items: &[BlockItem],
    cols: usize,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
) -> (Vec<Laid>, f64, f64) {
    let mut laid = Vec::new();
    let mut col = 0usize;
    let cols = cols.max(1);
    // Rows carry their own height so a composite taller than one cell pushes the
    // next row down instead of overlapping it (#310).
    let mut row_y = y0;
    let mut row_h: f64 = 0.0;
    // Advance to the next row, growing `row_y` by the tallest cell placed so far.
    macro_rules! wrap_row {
        () => {{
            col = 0;
            row_y += row_h + GAP;
            row_h = 0.0;
        }};
    }

    for item in items {
        match item {
            BlockItem::Block(b) => {
                let span = b.span.max(1);
                if col + span > cols && col != 0 {
                    wrap_row!();
                }
                let x = x0 + col as f64 * (cw + GAP);
                let w = span as f64 * cw + (span - 1) as f64 * GAP;
                laid.push(Laid {
                    item: item.clone(),
                    x,
                    y: row_y,
                    w,
                    h: ch,
                    children: Vec::new(),
                    child_tf: None,
                });
                row_h = row_h.max(ch);
                col += span;
                if col >= cols {
                    wrap_row!();
                }
            }
            BlockItem::Space(n) => {
                col += n;
                if col >= cols {
                    wrap_row!();
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
                // A composite block hugs its children at their natural size plus
                // an inner pad on every side — it never shrinks them below text
                // size (#310, correcting #259's over-compaction). It then claims
                // as many whole grid columns as it needs, so a sibling wraps
                // rather than overlapping it.
                let (child_laid, content_w, content_h) =
                    layout_items(&g.items, g.columns.unwrap_or(cols), 0.0, 0.0, cw, ch);
                let inner_w = content_w + GROUP_PAD * 2.0;
                let inner_h = content_h + GROUP_PAD * 2.0;
                let need = ((inner_w + GAP) / (cw + GAP)).ceil() as usize;
                let span = need.max(g.span.max(1));
                if col + span > cols && col != 0 {
                    wrap_row!();
                }
                let x = x0 + col as f64 * (cw + GAP);
                let w = (span as f64 * cw + (span - 1) as f64 * GAP).max(inner_w);
                let h = inner_h;
                // Children keep their own frame at natural scale, centered in the
                // container with the inner pad above.
                let tf = Transform {
                    tx: x + (w - content_w) / 2.0,
                    ty: row_y + GROUP_PAD,
                    s: 1.0,
                };
                laid.push(Laid {
                    item: item.clone(),
                    x,
                    y: row_y,
                    w,
                    h,
                    children: child_laid,
                    child_tf: Some(tf),
                });
                row_h = row_h.max(h);
                col += span;
                if col >= cols {
                    wrap_row!();
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
